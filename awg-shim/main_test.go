package main

import (
	"strings"
	"testing"
)

func TestParseConfAmneziaWG31(t *testing.T) {
	conf := `[Interface]
Address = 10.8.1.7/32
PrivateKey = AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=
Jc = 5
H1 = 100-200
HeaderProtectionKey = AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=
ContentPaddingAddition = 16-64
RekeyAfterTime = 120-180
RekeyTimeout = 3-5
RejectAfterTime = 180-240
KeepaliveTimeout = 10-15
MaxHandshakeAttempts = 5-8
RandomTrailers = on
DisableCookies = off

[Peer]
PublicKey = AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=
Endpoint = 192.0.2.1:51820
AllowedIPs = 0.0.0.0/0
PersistentKeepalive = 20-30
`

	parsed, err := parseConf(conf)
	if err != nil {
		t.Fatalf("parseConf: %v", err)
	}

	want := []string{
		"h1=100-200",
		"header_protection_key=" + strings.Repeat("0", 64),
		"content_padding_addition=16-64",
		"rekey_after_time=120-180",
		"rekey_timeout=3-5",
		"reject_after_time=180-240",
		"keepalive_timeout=10-15",
		"max_handshake_attempts=5-8",
		"random_trailers=true",
		"disable_cookies=false",
		"persistent_keepalive_interval=20-30",
	}
	for _, item := range want {
		if !strings.Contains(parsed.uapi, item+"\n") {
			t.Errorf("UAPI does not contain %q:\n%s", item, parsed.uapi)
		}
	}

	offConf := strings.Replace(conf, "PersistentKeepalive = 20-30", "PersistentKeepalive = off", 1)
	offParsed, err := parseConf(offConf)
	if err != nil {
		t.Fatalf("parseConf with disabled keepalive: %v", err)
	}
	if !strings.Contains(offParsed.uapi, "persistent_keepalive_interval=0\n") {
		t.Errorf("disabled keepalive was not normalized:\n%s", offParsed.uapi)
	}
}

func TestAwgBoolRejectsUnknownValue(t *testing.T) {
	if _, err := awgBool("sometimes"); err == nil {
		t.Fatal("expected invalid boolean to fail")
	}
}
