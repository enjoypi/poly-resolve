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
pull)
  echo "$@" > "$FAKE_DIR/pull-args.txt"
  case "$FAKE_PULL" in
    fail) exit "${FAKE_CODE:-1}" ;;
    slow) exec sleep 30 ;;
    *) if [ -f "$FAKE_DIR/pull.json" ]; then cat "$FAKE_DIR/pull.json"; fi ;;
  esac
  ;;
esac
