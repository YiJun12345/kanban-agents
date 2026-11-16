#!/bin/sh
# A complete outside worker in a few lines of shell: it takes one waiting task from the
# board, does the "work", and hands it in. Replace the marked part with a real agent.
#
#   cargo build --manifest-path cli/Cargo.toml        # builds cli/target/debug/kanban
#   export PATH="$PWD/cli/target/debug:$PATH"
#   export KANBAN_TOKEN=...                            # from the Workers page, or `kanban register NAME`
#   sh examples/external-agent.sh
set -eu

job=$(kanban next)
if [ "$job" = "null" ]; then
  echo "nothing is waiting"
  exit 0
fi
id=$(echo "$job" | jq -r .task.id)
prompt=$(echo "$job" | jq -r .task.prompt)
feedback=$(echo "$job" | jq -r '.feedback // empty')
cd "$(echo "$job" | jq -r .worktreePath)"      # the task's own git worktree

kanban note "$id" --kind plan "Write the request into NOTES.md" > /dev/null
[ -z "$feedback" ] || kanban note "$id" --kind decision "Reviewer asked: $feedback" > /dev/null

# ---- the work: a real agent would edit the code here ----
echo "- $prompt" >> NOTES.md
kanban hb "$id" > /dev/null                    # long jobs: send this at least every 30 seconds
# ----------------------------------------------------------

kanban note "$id" --kind evidence "NOTES.md has $(wc -l < NOTES.md) line(s)" > /dev/null
kanban submit "$id" --summary "Added the request to NOTES.md"
