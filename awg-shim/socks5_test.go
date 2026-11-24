package awgshim

import (
	"bytes"
	"encoding/binary"
	"io"
	"net"
	"net/netip"
	"testing"
	"time"

	"github.com/amnezia-vpn/amneziawg-go/v3/tun/netstack"
)

func TestSocksRejectsUnofferedMethod(t *testing.T) {
	client := startTestSocksConnection(t, &socksServer{})
	if got := exchange(t, client, []byte{5, 1, authPassword}, 2); !bytes.Equal(got, []byte{5, authUnavailable}) {
		t.Fatalf("expected no acceptable authentication method, got %v", got)
	}
}

func TestProtectedSocksRejectsUnauthenticatedClients(t *testing.T) {
	t.Run("missing credentials", func(t *testing.T) {
		client := startTestSocksConnection(t, &socksServer{password: "test-secret"})
		if got := exchange(t, client, []byte{5, 1, authNone}, 2); !bytes.Equal(got, []byte{5, authUnavailable}) {
			t.Fatalf("expected no acceptable authentication method, got %v", got)
		}
	})
	for _, tc := range []struct {
		name     string
		user     string
		password string
	}{
		{"wrong username", "other", "test-secret"},
		{"wrong password", "unigate", "wrong-secret"},
	} {
		t.Run(tc.name, func(t *testing.T) {
			client := startTestSocksConnection(t, &socksServer{password: "test-secret"})
			expectSocksAuth(t, client, tc.user, tc.password, 1)
		})
	}
	t.Run("raw UDP relay disabled", func(t *testing.T) {
		client := startTestSocksConnection(t, &socksServer{password: "test-secret"})
		expectSocksAuth(t, client, "unigate", "test-secret", 0)
		got := exchange(t, client, []byte{5, cmdUDPAssociate, 0, 1, 0, 0, 0, 0, 0, 0}, 10)
		if got[1] != repCmdNotSupported {
			t.Fatalf("expected raw UDP relay to be refused, got %v", got)
		}
	})
}

func TestProtectedSocksUDPOverTCP(t *testing.T) {
	localIP := netip.MustParseAddr("10.8.0.1")
	tun, tunnel, err := netstack.CreateNetTUN([]netip.Addr{localIP}, nil, 1420)
	if err != nil {
		t.Fatal(err)
	}
	defer tun.Close()
	echo, err := tunnel.ListenUDPAddrPort(netip.AddrPortFrom(localIP, 0))
	if err != nil {
		t.Fatal(err)
	}
	defer echo.Close()
	go func() {
		data := make([]byte, 256)
		n, from, err := echo.ReadFrom(data)
		if err == nil {
			_, _ = echo.WriteTo(data[:n], from)
		}
	}()

	client := startTestSocksConnection(t, &socksServer{tnet: tunnel, password: "test-secret"})
	expectSocksAuth(t, client, "unigate", "test-secret", 0)
	host := "sp.udp-over-tcp.arpa"
	req := append([]byte{5, cmdConnect, 0, 3, byte(len(host))}, host...)
	if got := exchange(t, client, append(req, 0, 0), 10); got[1] != repOK {
		t.Fatalf("UoT CONNECT failed: %v", got)
	}
	destination := echo.LocalAddr().(*net.UDPAddr).AddrPort()
	frame := append([]byte{0}, localIP.AsSlice()...)
	frame = binary.BigEndian.AppendUint16(frame, destination.Port())
	payload := []byte("authenticated UDP round trip")
	frame = append(binary.BigEndian.AppendUint16(frame, uint16(len(payload))), payload...)
	if got := exchange(t, client, frame, len(frame)); !bytes.Equal(got, frame) {
		t.Fatalf("unexpected UDP reply: %v", got)
	}
}

func startTestSocksConnection(t *testing.T, handler *socksServer) net.Conn {
	t.Helper()
	client, server := net.Pipe()
	t.Cleanup(func() { _ = client.Close() })
	if err := client.SetDeadline(time.Now().Add(3 * time.Second)); err != nil {
		t.Fatal(err)
	}
	go handler.handle(server)
	return client
}

func expectSocksAuth(t *testing.T, c net.Conn, username, password string, status byte) {
	t.Helper()
	if got := exchange(t, c, []byte{5, 2, authNone, authPassword}, 2); !bytes.Equal(got, []byte{5, authPassword}) {
		t.Fatalf("expected password authentication, got %v", got)
	}
	request := append([]byte{1, byte(len(username))}, username...)
	request = append(append(request, byte(len(password))), password...)
	if got := exchange(t, c, request, 2); !bytes.Equal(got, []byte{1, status}) {
		t.Fatalf("expected authentication status %d, got %v", status, got)
	}
}

func exchange(t *testing.T, c net.Conn, request []byte, replySize int) []byte {
	t.Helper()
	if _, err := c.Write(request); err != nil {
		t.Fatal(err)
	}
	reply := make([]byte, replySize)
	if _, err := io.ReadFull(c, reply); err != nil {
		t.Fatal(err)
	}
	return reply
}
