// SOCKS5 server dialing through the AmneziaWG netstack.
package awgshim

import (
	"context"
	"crypto/subtle"
	"encoding/binary"
	"fmt"
	"io"
	"net"
	"net/netip"
	"slices"
	"strconv"
	"sync"
	"time"

	"github.com/amnezia-vpn/amneziawg-go/v3/tun/netstack"
)

const (
	cmdConnect      = 0x01
	cmdUDPAssociate = 0x03

	authNone        = 0x00
	authPassword    = 0x02
	authUnavailable = 0xff

	uotAddress = "sp.udp-over-tcp.arpa:0"

	atypIPv4   = 0x01
	atypDomain = 0x03
	atypIPv6   = 0x04

	repOK              = 0x00
	repGeneralFailure  = 0x01
	repHostUnreachable = 0x04
	repRefused         = 0x05
	repCmdNotSupported = 0x07

	udpIdleTimeout = 2 * time.Minute
)

type socksServer struct {
	tnet     *netstack.Net
	password string
}

func (s *socksServer) serve(ln net.Listener) error {
	for {
		c, err := ln.Accept()
		if err != nil {
			return err
		}
		go s.handle(c)
	}
}

func (s *socksServer) handle(c net.Conn) {
	defer c.Close()
	_ = c.SetDeadline(time.Now().Add(10 * time.Second))

	// greeting: VER NMETHODS METHODS...
	hdr := make([]byte, 2)
	if _, err := io.ReadFull(c, hdr); err != nil || hdr[0] != 0x05 {
		return
	}
	methods := make([]byte, hdr[1])
	if _, err := io.ReadFull(c, methods); err != nil {
		return
	}
	method := byte(authNone)
	if s.password != "" {
		method = authPassword
	}
	if !slices.Contains(methods, method) {
		_, _ = c.Write([]byte{0x05, authUnavailable})
		return
	}
	if _, err := c.Write([]byte{0x05, method}); err != nil {
		return
	}
	if method == authPassword && !s.authenticate(c) {
		return
	}

	// request: VER CMD RSV ATYP DST.ADDR DST.PORT
	req := make([]byte, 4)
	if _, err := io.ReadFull(c, req); err != nil || req[0] != 0x05 {
		return
	}
	dst, err := readAddr(c, req[3])
	if err != nil {
		_ = writeReply(c, repGeneralFailure)
		return
	}
	_ = c.SetDeadline(time.Time{})

	switch req[1] {
	case cmdConnect:
		if s.password != "" && dst == uotAddress {
			s.handleUOT(c)
		} else {
			s.handleConnect(c, dst)
		}
	case cmdUDPAssociate:
		if s.password != "" {
			_ = writeReply(c, repCmdNotSupported)
			return
		}
		s.handleUDP(c)
	default:
		_ = writeReply(c, repCmdNotSupported)
	}
}

func (s *socksServer) authenticate(c net.Conn) bool {
	var version [1]byte
	if _, err := io.ReadFull(c, version[:]); err != nil || version[0] != 0x01 {
		return false
	}
	username, err := readAuthField(c)
	if err != nil {
		return false
	}
	password, err := readAuthField(c)
	if err != nil {
		return false
	}
	status := byte(1)
	if string(username) == "unigate" && subtle.ConstantTimeCompare(password, []byte(s.password)) == 1 {
		status = 0
	}
	_, err = c.Write([]byte{0x01, status})
	return err == nil && status == 0
}

func readAuthField(c io.Reader) ([]byte, error) {
	var size [1]byte
	if _, err := io.ReadFull(c, size[:]); err != nil {
		return nil, err
	}
	value := make([]byte, size[0])
	_, err := io.ReadFull(c, value)
	return value, err
}

func (s *socksServer) handleConnect(c net.Conn, dst string) {
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	remote, err := s.tnet.DialContext(ctx, "tcp", dst)
	cancel()
	if err != nil {
		_ = writeReply(c, repHostUnreachable)
		return
	}
	defer remote.Close()
	if err := writeReply(c, repOK); err != nil {
		return
	}

	done := make(chan struct{}, 2)
	go func() { _, _ = io.Copy(remote, c); done <- struct{}{} }()
	go func() { _, _ = io.Copy(c, remote); done <- struct{}{} }()
	<-done // either direction closing tears the pair down
}

// ---------------------------------------------------------------------------
// UDP ASSOCIATE: host-side relay socket <-> per-destination netstack sockets
// ---------------------------------------------------------------------------

type udpAssoc struct {
	mu     sync.Mutex
	conns  map[netip.AddrPort]net.Conn // destination -> netstack socket
	closed bool
}

func (s *socksServer) handleUDP(c net.Conn) {
	relay, err := net.ListenUDP("udp4", &net.UDPAddr{IP: net.IPv4(127, 0, 0, 1)})
	if err != nil {
		_ = writeReply(c, repGeneralFailure)
		return
	}
	defer relay.Close()

	// reply with the relay address the client must send datagrams to
	bnd := relay.LocalAddr().(*net.UDPAddr)
	reply := []byte{0x05, repOK, 0x00, atypIPv4}
	reply = append(reply, bnd.IP.To4()...)
	reply = binary.BigEndian.AppendUint16(reply, uint16(bnd.Port))
	if _, err := c.Write(reply); err != nil {
		return
	}

	assoc := &udpAssoc{conns: map[netip.AddrPort]net.Conn{}}
	go s.relayLoop(relay, assoc)

	// the association lives as long as the control connection
	_, _ = io.Copy(io.Discard, c)
	relay.Close()
	assoc.close()
}

func (s *socksServer) relayLoop(relay *net.UDPConn, assoc *udpAssoc) {
	buf := make([]byte, 65535)
	var client *net.UDPAddr // first sender wins; sing-box uses one socket
	for {
		n, from, err := relay.ReadFromUDP(buf)
		if err != nil {
			return
		}
		if client == nil {
			client = from
		} else if !from.IP.Equal(client.IP) || from.Port != client.Port {
			continue
		}
		dst, payload, err := parseUDPHeader(buf[:n])
		if err != nil {
			continue
		}
		peer := client
		s.forwardUDP(assoc, dst, payload, func(from netip.AddrPort, data []byte) error {
			_, err := relay.WriteToUDP(append(udpHeader(from), data...), peer)
			return err
		})
	}
}

func (s *socksServer) handleUOT(c net.Conn) {
	if err := writeReply(c, repOK); err != nil {
		return
	}
	assoc := &udpAssoc{conns: make(map[netip.AddrPort]net.Conn)}
	defer assoc.close()
	payload := make([]byte, 65535)
	var writeMu sync.Mutex
	reply := func(from netip.AddrPort, data []byte) error {
		atyp := byte(0)
		if from.Addr().Is6() {
			atyp = 1
		}
		packet := append([]byte{atyp}, from.Addr().AsSlice()...)
		packet = binary.BigEndian.AppendUint16(packet, from.Port())
		packet = binary.BigEndian.AppendUint16(packet, uint16(len(data)))
		writeMu.Lock()
		defer writeMu.Unlock()
		_ = c.SetWriteDeadline(time.Now().Add(10 * time.Second))
		_, err := c.Write(append(packet, data...))
		return err
	}
	for {
		dst, length, err := readUOTHeader(c)
		if err != nil {
			return
		}
		if _, err := io.ReadFull(c, payload[:length]); err != nil {
			return
		}
		s.forwardUDP(assoc, dst, payload[:length], reply)
	}
}

// UoT v1 differs from the SOCKS address encoding only in the type byte.
// https://sing-box.sagernet.org/configuration/shared/udp-over-tcp/
func readUOTHeader(r io.Reader) (string, int, error) {
	var kind [1]byte
	if _, err := io.ReadFull(r, kind[:]); err != nil {
		return "", 0, err
	}
	kinds := [...]byte{atypIPv4, atypIPv6, atypDomain}
	if int(kind[0]) >= len(kinds) {
		return "", 0, fmt.Errorf("invalid UoT address type")
	}
	dst, err := readAddr(r, kinds[kind[0]])
	if err != nil {
		return "", 0, err
	}
	var size [2]byte
	if _, err := io.ReadFull(r, size[:]); err != nil {
		return "", 0, err
	}
	return dst, int(binary.BigEndian.Uint16(size[:])), nil
}

func (assoc *udpAssoc) close() {
	assoc.mu.Lock()
	defer assoc.mu.Unlock()
	assoc.closed = true
	for _, conn := range assoc.conns {
		_ = conn.Close()
	}
}

func (s *socksServer) forwardUDP(assoc *udpAssoc, dst string, payload []byte, reply func(netip.AddrPort, []byte) error) {
	addr, err := s.resolveUDP(dst)
	if err != nil {
		return
	}
	assoc.mu.Lock()
	if assoc.closed {
		assoc.mu.Unlock()
		return
	}
	conn := assoc.conns[addr]
	if conn == nil {
		conn, err = s.tnet.DialUDPAddrPort(netip.AddrPort{}, addr)
		if err != nil {
			assoc.mu.Unlock()
			return
		}
		assoc.conns[addr] = conn
		go pumpReplies(addr, conn, assoc, reply)
	}
	assoc.mu.Unlock()
	_ = conn.SetReadDeadline(time.Now().Add(udpIdleTimeout))
	_, _ = conn.Write(payload)
}

// pumpReplies exits on idle timeout or close.
func pumpReplies(dst netip.AddrPort, rc net.Conn, assoc *udpAssoc, reply func(netip.AddrPort, []byte) error) {
	defer func() {
		assoc.mu.Lock()
		delete(assoc.conns, dst)
		assoc.mu.Unlock()
		rc.Close()
	}()
	buf := make([]byte, 65535)
	for {
		_ = rc.SetReadDeadline(time.Now().Add(udpIdleTimeout))
		n, err := rc.Read(buf)
		if err != nil {
			return
		}
		if err := reply(dst, buf[:n]); err != nil {
			return
		}
	}
}

// resolveUDP turns a textual destination (possibly a domain) into ip:port,
// resolving through the tunnel's DNS.
func (s *socksServer) resolveUDP(dst string) (netip.AddrPort, error) {
	host, portStr, err := net.SplitHostPort(dst)
	if err != nil {
		return netip.AddrPort{}, err
	}
	port, err := strconv.Atoi(portStr)
	if err != nil {
		return netip.AddrPort{}, err
	}
	if addr, err := netip.ParseAddr(host); err == nil {
		return netip.AddrPortFrom(addr.Unmap(), uint16(port)), nil
	}
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	ips, err := s.tnet.LookupContextHost(ctx, host)
	if err != nil || len(ips) == 0 {
		return netip.AddrPort{}, fmt.Errorf("lookup %s: %v", host, err)
	}
	addr, err := netip.ParseAddr(ips[0])
	if err != nil {
		return netip.AddrPort{}, err
	}
	return netip.AddrPortFrom(addr.Unmap(), uint16(port)), nil
}

// ---------------------------------------------------------------------------
// wire helpers
// ---------------------------------------------------------------------------

// readAddr reads ATYP-specific DST.ADDR + DST.PORT and returns "host:port".
func readAddr(r io.Reader, atyp byte) (string, error) {
	switch atyp {
	case atypIPv4:
		b := make([]byte, 6)
		if _, err := io.ReadFull(r, b); err != nil {
			return "", err
		}
		return joinIPPort(net.IP(b[:4]), b[4:]), nil
	case atypIPv6:
		b := make([]byte, 18)
		if _, err := io.ReadFull(r, b); err != nil {
			return "", err
		}
		return joinIPPort(net.IP(b[:16]), b[16:]), nil
	case atypDomain:
		l := make([]byte, 1)
		if _, err := io.ReadFull(r, l); err != nil {
			return "", err
		}
		b := make([]byte, int(l[0])+2)
		if _, err := io.ReadFull(r, b); err != nil {
			return "", err
		}
		host := string(b[:l[0]])
		return net.JoinHostPort(host, strconv.Itoa(int(binary.BigEndian.Uint16(b[l[0]:])))), nil
	default:
		return "", fmt.Errorf("bad atyp %d", atyp)
	}
}

// parseUDPHeader parses a SOCKS5 UDP datagram: RSV(2) FRAG ATYP ADDR PORT DATA.
func parseUDPHeader(pkt []byte) (dst string, payload []byte, err error) {
	if len(pkt) < 4 {
		return "", nil, fmt.Errorf("short packet")
	}
	if pkt[2] != 0 {
		return "", nil, fmt.Errorf("fragmentation unsupported")
	}
	atyp := pkt[3]
	rest := pkt[4:]
	var addrLen int
	switch atyp {
	case atypIPv4:
		addrLen = 4
	case atypIPv6:
		addrLen = 16
	case atypDomain:
		if len(rest) < 1 {
			return "", nil, fmt.Errorf("short domain")
		}
		addrLen = 1 + int(rest[0])
	default:
		return "", nil, fmt.Errorf("bad atyp %d", atyp)
	}
	if len(rest) < addrLen+2 {
		return "", nil, fmt.Errorf("short addr")
	}
	port := strconv.Itoa(int(binary.BigEndian.Uint16(rest[addrLen : addrLen+2])))
	var host string
	if atyp == atypDomain {
		host = string(rest[1:addrLen])
	} else {
		host = net.IP(rest[:addrLen]).String()
	}
	return net.JoinHostPort(host, port), rest[addrLen+2:], nil
}

// udpHeader builds the SOCKS5 UDP header naming dst as the datagram source.
func udpHeader(dst netip.AddrPort) []byte {
	var h []byte
	if dst.Addr().Is4() {
		h = append([]byte{0, 0, 0, atypIPv4}, dst.Addr().AsSlice()...)
	} else {
		h = append([]byte{0, 0, 0, atypIPv6}, dst.Addr().AsSlice()...)
	}
	return binary.BigEndian.AppendUint16(h, dst.Port())
}

func joinIPPort(ip net.IP, portBytes []byte) string {
	return net.JoinHostPort(ip.String(), strconv.Itoa(int(binary.BigEndian.Uint16(portBytes))))
}

func writeReply(c net.Conn, rep byte) error {
	_, err := c.Write([]byte{0x05, rep, 0x00, atypIPv4, 0, 0, 0, 0, 0, 0})
	return err
}
