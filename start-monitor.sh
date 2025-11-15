#!/bin/bash
#
# Start the off-chain monitoring script in a detached screen session
#

set -e

cd /home/twzrd/milo-token

# Check if .env exists
if [ ! -f .env ]; then
    echo "❌ .env file not found!"
    echo "Please create .env with DATABASE_URL set."
    exit 1
fi

# Load environment
source .env

# Check if DATABASE_URL is set
if [ -z "$DATABASE_URL" ]; then
    echo "❌ DATABASE_URL not set in .env"
    exit 1
fi

echo "✅ Environment loaded: DATABASE_URL is set"
echo ""

# Check if screen is available, fall back to tmux
if command -v screen &> /dev/null; then
    SESSION_NAME="milo-monitor"

    # Kill existing session if it exists
    screen -S "$SESSION_NAME" -X quit 2>/dev/null || true
    sleep 1

    echo "Starting monitor in screen session: $SESSION_NAME"
    echo "To attach: screen -r $SESSION_NAME"
    echo "To detach: Ctrl+A, then D"
    echo ""

    # Create new session and run monitor
    screen -dmS "$SESSION_NAME" bash -c "cd /home/twzrd/milo-token && source .env && npx tsx apps/twzrd-aggregator/monitor.ts"

    # Wait a moment then show logs
    sleep 2
    screen -S "$SESSION_NAME" -X hardcopy -h /tmp/screen-monitor.txt
    tail -30 /tmp/screen-monitor.txt

    echo ""
    echo "✅ Monitor started successfully!"
    echo "View logs: screen -r $SESSION_NAME"
    echo "Check log file: tail -f ~/.pm2/monitor.log"

elif command -v tmux &> /dev/null; then
    SESSION_NAME="milo-monitor"

    # Kill existing session if it exists
    tmux kill-session -t "$SESSION_NAME" 2>/dev/null || true
    sleep 1

    echo "Starting monitor in tmux session: $SESSION_NAME"
    echo "To attach: tmux attach -t $SESSION_NAME"
    echo "To detach: Ctrl+B, then D"
    echo ""

    # Create new session and run monitor
    tmux new-session -d -s "$SESSION_NAME" -c /home/twzrd/milo-token \
        "source .env && npx tsx apps/twzrd-aggregator/monitor.ts"

    # Wait a moment then show logs
    sleep 2
    tmux capture-pane -t "$SESSION_NAME" -p | tail -30

    echo ""
    echo "✅ Monitor started successfully!"
    echo "View logs: tmux attach -t $SESSION_NAME"
    echo "Check log file: tail -f ~/.pm2/monitor.log"

else
    echo "❌ Neither screen nor tmux is available!"
    echo "Install one with:"
    echo "  apt-get install screen  # or"
    echo "  apt-get install tmux"
    exit 1
fi
