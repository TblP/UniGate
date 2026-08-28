// awg-shim: userspace AmneziaWG tunnel exposed as a local SOCKS5 proxy
// (TCP CONNECT + UDP ASSOCIATE). No TUN device, no elevated privileges.
//
// UniGate runs this next to sing-box: sing-box does all routing (TUN or
// mixed inbound) and forwards the "via VPN" traffic to this proxy, which
// speaks AmneziaWG to the server through gVisor netstack.
//
// The desktop command lives in cmd/awg-shim. This package is also bound into
// an Android AAR with gomobile, so both platforms use exactly the same parser,
// AWG 3.1 engine and SOCKS implementation.
package awgshim

import (
	"encoding/base64"
	"encoding/hex"
	"fmt"
	"net"
	"net/netip"
	"strconv"
	"strings"
	"sync"

	"github.com/amnezia-vpn/amneziawg-go/v3/conn"
	"github.com/amnezia-vpn/amneziawg-go/v3/device"
	"github.com/amnezia-vpn/amneziawg-go/v3/tun/netstack"
)

// Protector is implemented by Android's VpnService. The AmneziaWG transport
// sockets must be excluded from the system VPN, otherwise they loop back into
// sing-box. Desktop callers pass nil and use their normal direct-route rule.
type Protector interface {
	Protect(fd int) bool
}

type engine struct {
	device   *device.Device
	listener net.Listener
}

var active struct {
	sync.Mutex
	engine *engine
}

// Start launches the userspace AmneziaWG engine and its local SOCKS5 server.
// It returns the actual listen address after the engine is ready.
func Start(configText, listen string, protector Protector) (string, error) {
	Stop()
	if strings.TrimSpace(listen) == "" {
		listen = "127.0.0.1:2081"
	}
	cfg, err := parseConf(configText)
	if err != nil {
		return "", fmt.Errorf("parse conf: %w", err)
	}

	tunDev, tnet, err := netstack.CreateNetTUN(cfg.addresses, cfg.dns, cfg.mtu)
	if err != nil {
		return "", fmt.Errorf("netstack: %w", err)
	}
	bind := conn.NewDefaultBind()
	if protector != nil {
		bind = &protectedBind{Bind: bind, protector: protector}
	}
	dev := device.NewDevice(tunDev, bind, device.NewLogger(device.LogLevelError, "awg "))
	if err := dev.IpcSet(cfg.uapi); err != nil {
		dev.Close()
		return "", fmt.Errorf("device config: %w", err)
	}
	if err := dev.Up(); err != nil {
		dev.Close()
		return "", fmt.Errorf("device up: %w", err)
	}

	srv := &socksServer{tnet: tnet}
	ln, err := net.Listen("tcp", listen)
	if err != nil {
		dev.Close()
		return "", fmt.Errorf("listen %s: %w", listen, err)
	}
	instance := &engine{device: dev, listener: ln}
	active.Lock()
	active.engine = instance
	active.Unlock()
	go func() { _ = srv.serve(ln) }()
	return ln.Addr().String(), nil
}

// Stop shuts down both SOCKS and the AWG device. It is safe to call repeatedly.
func Stop() {
	active.Lock()
	instance := active.engine
	active.engine = nil
	active.Unlock()
	if instance != nil {
		_ = instance.listener.Close()
		instance.device.Close()
	}
}

type protectedBind struct {
	conn.Bind
	protector Protector
}

func (b *protectedBind) Open(port uint16) ([]conn.ReceiveFunc, uint16, error) {
	receivers, actualPort, err := b.Bind.Open(port)
	if err != nil {
		return nil, 0, err
	}
	peek, ok := b.Bind.(conn.PeekLookAtSocketFd)
	if !ok {
		_ = b.Bind.Close()
		return nil, 0, fmt.Errorf("AmneziaWG bind does not expose socket descriptors")
	}
	protected := false
	for _, getFD := range []func() (int, error){peek.PeekLookAtSocketFd4, peek.PeekLookAtSocketFd6} {
		fd, fdErr := getFD()
		if fdErr != nil {
			continue
		}
		if !b.protector.Protect(fd) {
			_ = b.Bind.Close()
			return nil, 0, fmt.Errorf("Android refused to protect AmneziaWG socket %d", fd)
		}
		protected = true
	}
	if !protected {
		_ = b.Bind.Close()
		return nil, 0, fmt.Errorf("AmneziaWG did not open an IPv4 or IPv6 socket")
	}
	return receivers, actualPort, nil
}

// ---------------------------------------------------------------------------
// wg-quick style conf → netstack params + UAPI config text
// ---------------------------------------------------------------------------

type shimConf struct {
	addresses []netip.Addr
	dns       []netip.Addr
	mtu       int
	uapi      string
}

type awgField struct {
	conf string
	uapi string
}

// AmneziaWG interface fields. Config names are normalized to lower-case by
// parseConf; UAPI uses snake_case for the 3.x parameters.
var awgFields = []awgField{
	{"jc", "jc"}, {"jmin", "jmin"}, {"jmax", "jmax"},
	{"s1", "s1"}, {"s2", "s2"}, {"s3", "s3"}, {"s4", "s4"},
	{"h1", "h1"}, {"h2", "h2"}, {"h3", "h3"}, {"h4", "h4"},
	{"i1", "i1"}, {"i2", "i2"}, {"i3", "i3"}, {"i4", "i4"}, {"i5", "i5"},
	{"headerprotectionkey", "header_protection_key"},
	{"contentpaddingaddition", "content_padding_addition"},
	{"rekeyaftertime", "rekey_after_time"},
	{"rekeytimeout", "rekey_timeout"},
	{"rejectaftertime", "reject_after_time"},
	{"keepalivetimeout", "keepalive_timeout"},
	{"maxhandshakeattempts", "max_handshake_attempts"},
	{"randomtrailers", "random_trailers"},
	{"disablecookies", "disable_cookies"},
}

func parseConf(text string) (*shimConf, error) {
	c := &shimConf{mtu: 1420}
	iface := map[string]string{}
	peer := map[string]string{}
	var cur map[string]string

	for _, line := range strings.Split(text, "\n") {
		line = strings.TrimSpace(line)
		if line == "" || strings.HasPrefix(line, "#") || strings.HasPrefix(line, ";") {
			continue
		}
		switch strings.ToLower(line) {
		case "[interface]":
			cur = iface
			continue
		case "[peer]":
			cur = peer
			continue
		}
		if cur == nil {
			continue
		}
		k, v, ok := strings.Cut(line, "=")
		if !ok {
			continue
		}
		cur[strings.ToLower(strings.TrimSpace(k))] = strings.TrimSpace(v)
	}

	// --- netstack parameters ---
	for _, a := range splitList(iface["address"]) {
		a, _, _ = strings.Cut(a, "/")
		addr, err := netip.ParseAddr(a)
		if err != nil {
			return nil, fmt.Errorf("Address %q: %w", a, err)
		}
		c.addresses = append(c.addresses, addr)
	}
	if len(c.addresses) == 0 {
		return nil, fmt.Errorf("no Address in [Interface]")
	}
	for _, d := range splitList(iface["dns"]) {
		addr, err := netip.ParseAddr(d)
		if err != nil {
			continue // e.g. search domains — not our concern
		}
		c.dns = append(c.dns, addr)
	}
	if len(c.dns) == 0 {
		c.dns = []netip.Addr{netip.MustParseAddr("8.8.8.8")}
	}
	if m := iface["mtu"]; m != "" {
		mtu, err := strconv.Atoi(m)
		if err != nil {
			return nil, fmt.Errorf("MTU %q: %w", m, err)
		}
		c.mtu = mtu
	}

	// --- UAPI: device section first, then peer ---
	var b strings.Builder
	priv, err := b64ToHex(iface["privatekey"])
	if err != nil {
		return nil, fmt.Errorf("PrivateKey: %w", err)
	}
	fmt.Fprintf(&b, "private_key=%s\n", priv)
	for _, field := range awgFields {
		if v, ok := iface[field.conf]; ok && v != "" {
			switch field.conf {
			case "headerprotectionkey":
				v, err = awgKeyToHex(v)
				if err != nil {
					return nil, fmt.Errorf("HeaderProtectionKey: %w", err)
				}
			case "randomtrailers", "disablecookies":
				v, err = awgBool(v)
				if err != nil {
					return nil, fmt.Errorf("%s: %w", field.conf, err)
				}
			}
			fmt.Fprintf(&b, "%s=%s\n", field.uapi, v)
		}
	}

	pub, err := b64ToHex(peer["publickey"])
	if err != nil {
		return nil, fmt.Errorf("PublicKey: %w", err)
	}
	fmt.Fprintf(&b, "public_key=%s\n", pub)
	if psk := peer["presharedkey"]; psk != "" {
		h, err := b64ToHex(psk)
		if err != nil {
			return nil, fmt.Errorf("PresharedKey: %w", err)
		}
		fmt.Fprintf(&b, "preshared_key=%s\n", h)
	}
	ep, err := resolveEndpoint(peer["endpoint"])
	if err != nil {
		return nil, fmt.Errorf("Endpoint: %w", err)
	}
	fmt.Fprintf(&b, "endpoint=%s\n", ep)
	if ka := peer["persistentkeepalive"]; ka != "" {
		if strings.EqualFold(ka, "off") {
			ka = "0"
		}
		fmt.Fprintf(&b, "persistent_keepalive_interval=%s\n", ka)
	}
	allowed := splitList(peer["allowedips"])
	if len(allowed) == 0 {
		allowed = []string{"0.0.0.0/0", "::/0"}
	}
	for _, a := range allowed {
		fmt.Fprintf(&b, "allowed_ip=%s\n", a)
	}

	c.uapi = b.String()
	return c, nil
}

func splitList(s string) []string {
	var out []string
	for _, p := range strings.Split(s, ",") {
		if p = strings.TrimSpace(p); p != "" {
			out = append(out, p)
		}
	}
	return out
}

func b64ToHex(s string) (string, error) {
	if s == "" {
		return "", fmt.Errorf("missing")
	}
	raw, err := base64.StdEncoding.DecodeString(s)
	if err != nil {
		return "", err
	}
	if len(raw) != 32 {
		return "", fmt.Errorf("bad key length %d", len(raw))
	}
	return hex.EncodeToString(raw), nil
}

func awgKeyToHex(s string) (string, error) {
	s = strings.TrimSpace(s)
	if len(s) == 64 {
		if raw, err := hex.DecodeString(s); err == nil && len(raw) == 32 {
			return strings.ToLower(s), nil
		}
	}
	return b64ToHex(s)
}

func awgBool(s string) (string, error) {
	switch strings.ToLower(strings.TrimSpace(s)) {
	case "on", "true", "1":
		return "true", nil
	case "off", "false", "0":
		return "false", nil
	default:
		return "", fmt.Errorf("expected on/off, true/false or 1/0")
	}
}

// UAPI wants ip:port; the conf may carry a hostname — resolve it with the
// system resolver (the shim's own UDP goes out via the physical interface;
// sing-box routing excludes the shim process / endpoint from the tunnel).
func resolveEndpoint(ep string) (string, error) {
	if ep == "" {
		return "", fmt.Errorf("missing")
	}
	host, port, err := net.SplitHostPort(ep)
	if err != nil {
		return "", err
	}
	if ip := net.ParseIP(host); ip != nil {
		return ep, nil
	}
	ips, err := net.LookupIP(host)
	if err != nil {
		return "", err
	}
	var pick net.IP
	for _, ip := range ips {
		if v4 := ip.To4(); v4 != nil {
			pick = v4
			break
		}
		if pick == nil {
			pick = ip
		}
	}
	if pick == nil {
		return "", fmt.Errorf("no addresses for %s", host)
	}
	return net.JoinHostPort(pick.String(), port), nil
}
