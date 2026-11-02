#!/bin/sh
# Run the command given as arguments, print its exit status as `exit N`,
# and exit with that status, so the transcript shows the status the
# scene's `? N` line expects. Standard input passes through untouched.
"$@"
status=$?
echo "exit $status"
exit $status
