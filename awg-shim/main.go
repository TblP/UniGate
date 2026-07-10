// awg-shim: userspace AmneziaWG tunnel exposed as a local SOCKS5 proxy
// (TCP CONNECT + UDP ASSOCIATE). No TUN device, no elevated privileges.
//
// UniGate runs this next to sing-box: sing-box does all routing (TUN or
// mixed inbound) and forwards the "via VPN" traffic to this proxy, which
// speaks AmneziaWG to the server through gVisor netstack.
//
// Usage: awg-shim --conf profile.conf [--listen 127.0.0.1:2081] [--verbose]
//
// The process exits when stdin reaches EOF (the parent holds a pipe), so an
// orphaned shim never outlives UniGate.
package main

import (
	"encoding/base64"
	"encoding/hex"
	"flag"
	"fmt"
	"io"
	"net"
	"net/netip"
	"os"
	"os/signal"
	"strconv"
	"strings"

	"github.com/amnezia-vpn/amneziawg-go/conn"
	"github.com/amnezia-vpn/amneziawg-go/device"
	"github.com/amnezia-vpn/amneziawg-go/tun/netstack"
)

func main() {
	confPath := flag.String("conf", "", "path to AmneziaWG/WireGuard .conf (wg-quick format)")
	listen := flag.String("listen", "127.0.0.1:2081", "SOCKS5 listen address")
	verbose := flag.Bool("verbose", false, "verbose device log")
	flag.Parse()

	if *confPath == "" {
		fatal("missing --conf")
	}
	raw, err := os.ReadFile(*confPath)
	if err != nil {
		fatal("read conf: %v", err)
	}
	cfg, err := parseConf(string(raw))
	if err != nil {
		fatal("parse conf: %v", err)
	}

	tunDev, tnet, err := netstack.CreateNetTUN(cfg.addresses, cfg.dns, cfg.mtu)
	if err != nil {
		fatal("netstack: %v", err)
	}
	logLevel := device.LogLevelError
	if *verbose {
		logLevel = device.LogLevelVerbose
	}
	dev := device.NewDevice(tunDev, conn.NewDefaultBind(), device.NewLogger(logLevel, "awg "))
	if err := dev.IpcSet(cfg.uapi); err != nil {
		fatal("device config: %v", err)
	}
	if err := dev.Up(); err != nil {
		fatal("device up: %v", err)
	}

	srv := &socksServer{tnet: tnet}
	ln, err := net.Listen("tcp", *listen)
	if err != nil {
		fatal("listen %s: %v", *listen, err)
	}
	// parent waits for this line before starting sing-box
	fmt.Printf("READY %s\n", ln.Addr())

	// lifecycle: die with the parent (stdin EOF) or on Ctrl+C
	go func() {
		_, _ = io.Copy(io.Discard, os.Stdin)
		dev.Close()
		os.Exit(0)
	}()
	sig := make(chan os.Signal, 1)
	signal.Notify(sig, os.Interrupt)
	go func() {
		<-sig
		dev.Close()
		os.Exit(0)
	}()

	if err := srv.serve(ln); err != nil {
		fatal("socks5: %v", err)
	}
}

func fatal(format string, args ...any) {
	fmt.Fprintf(os.Stderr, "FATAL "+format+"\n", args...)
	os.Exit(1)
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

// AmneziaWG obfuscation keys passed through to UAPI verbatim (values are
// numbers, "a-b" ranges for h1..h4 or "<b 0x..>" packet specs for i1..i5).
var awgKeys = []string{
	"jc", "jmin", "jmax",
	"s1", "s2", "s3", "s4",
	"h1", "h2", "h3", "h4",
	"i1", "i2", "i3", "i4", "i5",
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
	for _, k := range awgKeys {
		if v, ok := iface[k]; ok && v != "" {
			fmt.Fprintf(&b, "%s=%s\n", k, v)
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
