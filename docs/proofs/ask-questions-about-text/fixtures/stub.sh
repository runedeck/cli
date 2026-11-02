#!/bin/sh
# Start or stop the System One stub in the scene's directory.
#
#   sh fixtures/stub.sh start [stub options...]
#       runs systemone-stub.py in the background; writes stub.pid, stub.port,
#       stub.log, and requests.jsonl (one line per request) in the current
#       directory, and returns once the port is bound
#   sh fixtures/stub.sh stop
#       stops it and says how many requests it saw
set -eu

here=$(cd "$(dirname "$0")" && pwd)
case $1 in
start)
    shift
    rm -f stub.port requests.jsonl
    python3 "$here/systemone-stub.py" --port-file stub.port --record requests.jsonl "$@" >stub.log 2>&1 &
    echo $! >stub.pid
    tries=0
    while [ ! -s stub.port ]; do
        sleep 0.1
        tries=$((tries + 1))
        if [ "$tries" -ge 100 ]; then
            echo "the stub did not start" >&2
            cat stub.log >&2
            exit 1
        fi
    done
    echo "stub listening on 127.0.0.1:$(cat stub.port)"
    ;;
stop)
    kill "$(cat stub.pid)"
    count=$(wc -l <requests.jsonl | tr -d ' ')
    case $count in
    1) echo "stub stopped after 1 request" ;;
    *) echo "stub stopped after $count requests" ;;
    esac
    ;;
*)
    echo "usage: stub.sh start [options] | stop" >&2
    exit 2
    ;;
esac
