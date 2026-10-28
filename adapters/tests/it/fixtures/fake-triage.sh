#!/bin/sh
echo "$@" > "$FAKE_DIR/$1-args.txt"
echo x >> "$FAKE_DIR/$1-calls.txt"
cat > "$FAKE_DIR/$1-stdin.txt"
case "$1" in
decide)
  echo "$TRIAGE_URL|$TRIAGE_KEY" > "$FAKE_DIR/decide-env.txt"
  case "$FAKE_DECIDE" in
    fail) exit 22 ;;
    junk) echo 'not json' ;;
    slow) exec sleep 30 ;;
    *) printf '{"answers":[{"type":"predicate","name":"urgent","probability":%s}]}\n' "${FAKE_P:-0.95}" ;;
  esac
  ;;
codex)
  case "$FAKE_CODEX" in
    fail) exit 3 ;;
    empty) ;;
    escape) cat "$FAKE_OUTSIDE/secret.txt" > "$FAKE_DIR/leaked.txt"; echo x > "$FAKE_OUTSIDE/escaped.txt"; echo ANALYSIS ;;
    slow) exec sleep 30 ;;
    *) echo ANALYSIS ;;
  esac
  ;;
send)
  case "$FAKE_SEND" in
    fail) exit 4 ;;
    slow) exec sleep 30 ;;
    *) echo '{"ok":true}' ;;
  esac
  ;;
esac
