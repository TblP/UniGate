// Package xraybridge is a standalone proof of concept; it owns no TUN interface.
package xraybridge

import (
	"errors"
	"strings"
	"sync"
	"syscall"

	"github.com/xtls/xray-core/core"
	"github.com/xtls/xray-core/infra/conf/serial"
	_ "github.com/xtls/xray-core/main/distro/all"
	"github.com/xtls/xray-core/transport/internet"
)

type Protector interface {
	Protect(fd int64) bool
}

var (
	lifecycle   sync.Mutex
	instance    *core.Instance
	register    sync.Once
	registerErr error
	protectMu   sync.RWMutex
	protector   Protector
)

func setProtector(p Protector) {
	protectMu.Lock()
	protector = p
	protectMu.Unlock()
}

func protectSocket(_, _ string, raw syscall.RawConn) error {
	protectMu.RLock()
	p := protector
	protectMu.RUnlock()
	if p == nil {
		return errors.New("Xray socket protection unavailable")
	}
	var ok bool
	if err := raw.Control(func(fd uintptr) { ok = p.Protect(int64(fd)) }); err != nil {
		return err
	}
	if !ok {
		return errors.New("Xray socket protection failed")
	}
	return nil
}

// Start uses a UniGate-generated configuration. Errors omit profile contents.
func Start(config string, p Protector) error {
	lifecycle.Lock()
	defer lifecycle.Unlock()
	if p == nil || instance != nil {
		return errors.New("Xray requires a VPN socket protector and no running instance")
	}
	cfg, err := serial.LoadJSONConfig(strings.NewReader(config))
	if err != nil {
		return errors.New("invalid Xray helper configuration")
	}
	register.Do(func() { registerErr = internet.RegisterDialerController(protectSocket) })
	if registerErr != nil {
		return errors.New("could not register Xray socket protection")
	}
	setProtector(p)
	next, err := core.New(cfg)
	if err != nil {
		setProtector(nil)
		return errors.New("could not initialize Xray helper")
	}
	if err := next.Start(); err != nil {
		_ = next.Close()
		setProtector(nil)
		return errors.New("could not start Xray helper")
	}
	instance = next
	return nil
}

func Stop() error {
	lifecycle.Lock()
	defer lifecycle.Unlock()
	setProtector(nil)
	if instance == nil {
		return nil
	}
	if err := instance.Close(); err != nil {
		return errors.New("could not stop Xray helper")
	}
	instance = nil
	return nil
}
