// Command-line wrapper for the shared UniGate AmneziaWG shim.
package main

import (
	"flag"
	"fmt"
	"io"
	"os"
	"os/signal"

	awgshim "unigate/awg-shim"
)

func main() {
	confPath := flag.String("conf", "", "path to AmneziaWG/WireGuard .conf")
	listen := flag.String("listen", "127.0.0.1:2081", "SOCKS5 listen address")
	flag.Bool("verbose", false, "reserved for compatibility")
	flag.Parse()
	if *confPath == "" {
		fatal("missing --conf")
	}
	raw, err := os.ReadFile(*confPath)
	if err != nil {
		fatal("read conf: %v", err)
	}
	addr, err := awgshim.Start(string(raw), *listen, nil)
	if err != nil {
		fatal("%v", err)
	}
	fmt.Printf("READY %s\n", addr)

	done := make(chan struct{})
	go func() {
		_, _ = io.Copy(io.Discard, os.Stdin)
		close(done)
	}()
	sig := make(chan os.Signal, 1)
	signal.Notify(sig, os.Interrupt)
	select {
	case <-done:
	case <-sig:
	}
	awgshim.Stop()
}

func fatal(format string, args ...any) {
	fmt.Fprintf(os.Stderr, "FATAL "+format+"\n", args...)
	os.Exit(1)
}
