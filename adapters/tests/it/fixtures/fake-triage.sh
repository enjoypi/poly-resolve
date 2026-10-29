#!/bin/sh
emit() {
  printf '{"type":"thread.started","thread_id":"%s"}\n' "${FAKE_THREAD:-T1}"
  printf '{"type":"item.completed","item":{"id":"i","type":"agent_message","text":"ANALYSIS"}}\n'
}
answers() {
  grep -o '"name":"[a-z]*_[0-9]*"' "$FAKE_DIR/decide-stdin.txt" | cut -d'"' -f4 | awk -v p="${FAKE_P:-0.95}" -v ps="$FAKE_PS" -v ts="$FAKE_TOPICS" -v c="${FAKE_CONFIDENCE:-0.9}" '
    BEGIN { split(ps, probs, " "); split(ts, picks, " "); printf "{\"answers\":[" }
    {
      if (NR > 1) printf ","
      split($0, parts, "_")
      n = parts[2] + 0
      if (parts[1] == "urgent") {
        q = (n in probs) ? probs[n] : p
        printf "{\"type\":\"predicate\",\"name\":\"%s\",\"probability\":%s}", $0, q
      } else {
        v = (n in picks) ? picks[n] : "new"
        printf "{\"type\":\"choice\",\"name\":\"%s\",\"choice\":\"%s\",\"confidence\":%s}", $0, v, c
      }
    }
    END { print "]}" }'
}
verdicts() {
  if [ -f "$FAKE_DIR/screen.json" ]; then
    sed 's/"/\\"/g' "$FAKE_DIR/screen.json" | tr -d '\n' | awk '{ printf "{\"choices\":[{\"message\":{\"content\":\"%s\"}}]}\n", $0 }'
    return
  fi
  grep -o 'line[^L]*L[0-9]*' "$FAKE_DIR/screen-stdin.txt" | grep -o 'L[0-9]*$' | awk 'BEGIN { printf "{\"choices\":[{\"message\":{\"content\":\"{\\\"lines\\\":[" } { if (NR > 1) printf ","; printf "{\\\"line\\\":\\\"%s\\\",\\\"risk\\\":\\\"low\\\",\\\"spans\\\":[],\\\"topics\\\":[]}", $0 } END { print "]}\"}}]}" }'
}
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
    *) answers ;;
  esac
  ;;
codex)
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
screen)
  case "$FAKE_SCREEN" in
    fail) exit 6 ;;
    junk) echo 'not json' ;;
    slow) exec sleep 30 ;;
    *) verdicts ;;
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
