#!/bin/sh
emit() {
  printf '{"type":"thread.started","thread_id":"%s"}\n' "$thread"
  printf '{"type":"item.completed","item":{"id":"i","type":"agent_message","text":"ANALYSIS"}}\n'
}
echo "$@" > "$FAKE_DIR/$1-args.txt"
echo x >> "$FAKE_DIR/$1-calls.txt"
cat > "$FAKE_DIR/$1-stdin.txt"
case "$1" in
codex)
  case "$*" in
    *resume*) thread=$3 ;;
    *)
      if [ -n "$FAKE_THREAD" ]; then
        thread=$FAKE_THREAD
      else
        if [ -f "$FAKE_DIR/codex-next.txt" ]; then
          n=$(cat "$FAKE_DIR/codex-next.txt")
        else
          n=1
        fi
        thread=T$n
        printf '%s\n' $((n + 1)) > "$FAKE_DIR/codex-next.txt"
      fi
      ;;
  esac
  case "$FAKE_CODEX" in
    fail) exit 3 ;;
    empty) ;;
    junk) echo 'not an event' ;;
    resume_fail) case "$*" in *resume*) exit 5 ;; esac; emit ;;
    escape) cat "$FAKE_OUTSIDE/secret.txt" > "$FAKE_DIR/leaked.txt"; echo x > "$FAKE_OUTSIDE/escaped.txt"; emit ;;
    slow) exec sleep 30 ;;
    *) emit ;;
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
