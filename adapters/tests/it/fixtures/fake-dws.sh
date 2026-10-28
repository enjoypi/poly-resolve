#!/bin/sh
case "$1" in
listen)
  if [ -f "$FAKE_DIR/listen.txt" ]; then cat "$FAKE_DIR/listen.txt"; fi
  case "$FAKE_LISTEN" in
    exit) exit "${FAKE_CODE:-0}" ;;
    hang) exec sleep 30 ;;
    *) cat >/dev/null; exit 0 ;;
  esac
  ;;
lookup)
  echo "$@" >> "$FAKE_DIR/lookup-args.txt"
  case "$FAKE_LOOKUP" in
    fail) exit 1 ;;
    junk) echo 'not json' ;;
    slow) exec sleep 30 ;;
    *) if [ -f "$FAKE_DIR/lookup.json" ]; then cat "$FAKE_DIR/lookup.json"; fi ;;
  esac
  ;;
decrypt)
  echo "$@" > "$FAKE_DIR/decrypt-args.txt"
  case "$FAKE_DECRYPT" in
    fail) echo '{"error":{"code":5,"message":"decrypt failed"}}'; exit 5 ;;
    junk) echo 'not json' ;;
    slow) exec sleep 30 ;;
    *) echo '{"ok":true,"plaintext":"CLEARTEXT"}' ;;
  esac
  ;;
pull)
  echo "$@" > "$FAKE_DIR/pull-args.txt"
  case "$FAKE_PULL" in
    fail) exit "${FAKE_CODE:-1}" ;;
    slow) exec sleep 30 ;;
    escape) cat "$FAKE_OUTSIDE/secret.txt" > "$FAKE_DIR/leaked.txt"; echo x > "$FAKE_OUTSIDE/escaped.txt" ;;
    net) /usr/bin/curl -sS -m 2 "http://127.0.0.1:$FAKE_PORT/" > "$FAKE_DIR/net.txt" ;;
    *) if [ -f "$FAKE_DIR/pull.json" ]; then cat "$FAKE_DIR/pull.json"; fi ;;
  esac
  ;;
esac
