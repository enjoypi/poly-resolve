#!/bin/sh
emit() {
  printf '{"type":"thread.started","thread_id":"%s"}\n' "$thread"
  printf '{"type":"item.completed","item":{"id":"i","type":"agent_message","text":"%s\\nANALYSIS"}}\n' "$header"
  printf '{"type":"turn.completed","usage":{"input_tokens":20,"output_tokens":5}}\n'
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
  header='{\"changed\":true,\"status\":\"open\"}'
  case "$FAKE_CODEX" in
    titled) header='{\"changed\":true,\"changes\":\"moved\",\"status\":\"open\",\"title\":\"TITLE\"}'; emit ;;
    same) header='{\"changed\":false,\"status\":\"open\"}'; emit ;;
    resolved) header='{\"changed\":true,\"status\":\"resolved\"}'; emit ;;
    fail) exit 3 ;;
    empty) ;;
    junk) echo 'not an event' ;;
    resume_fail) case "$*" in *resume*) exit 5 ;; esac; emit ;;
    escape) cat "$FAKE_OUTSIDE/secret.txt" > "$FAKE_DIR/leaked.txt"; echo x > "$FAKE_OUTSIDE/escaped.txt"; emit ;;
    slow) exec sleep 30 ;;
    *) emit ;;
  esac
  ;;
purge)
  case "$FAKE_PURGE" in
    fail) exit 6 ;;
    slow) exec sleep 30 ;;
    *) echo ok ;;
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
