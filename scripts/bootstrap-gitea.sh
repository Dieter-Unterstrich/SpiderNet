#!/usr/bin/env sh
# Bootstrap: creates the repo on Gitea (via API) and pushes this project.
#
# Requirements:
#   - python3 (for parsing/decoding credentials from ~/.git-credentials)
#   - curl, git
#
# The credentials live in ~/.git-credentials (credential-helper "store").
# This script NEVER prints credentials.
#
# Only touches the repository named in REPO_NAME. No other repos on the
# Gitea server are created, modified or deleted.

set -eu

HOST="git.dieterunterstrich.de"
REPO_NAME="anarcho-kommunistisches-internet"
DESCRIPTION="Nachbarschafts-Netz: gepoolte Bandbreite, Gemeingut, Privacy by Design"
CRED_FILE="${HOME}/.git-credentials"

command -v python3 >/dev/null 2>&1 || { echo "ERROR: python3 required" >&2; exit 1; }
command -v curl >/dev/null 2>&1 || { echo "ERROR: curl required" >&2; exit 1; }

if [ ! -f "$CRED_FILE" ]; then
    echo "ERROR: $CRED_FILE not found" >&2
    exit 1
fi

# Parse credentials for HOST (no output of secrets)
set -- $(python3 - "$CRED_FILE" "$HOST" <<'PYEOF'
import sys, urllib.parse, shlex
cred_file, host = sys.argv[1], sys.argv[2]
line = None
with open(cred_file) as f:
    for raw in f:
        if host in raw.strip():
            line = raw.strip()
if not line:
    sys.exit(3)
u = urllib.parse.urlparse(line)
user = urllib.parse.unquote(u.username or "")
pw = urllib.parse.unquote(u.password or "")
print(shlex.quote(user))
print(shlex.quote(pw))
PYEOF
) || { echo "ERROR: no credentials for $HOST in $CRED_FILE" >&2; exit 1; }

GITEA_USER="$1"
GITEA_PASS="$2"

if [ -z "$GITEA_USER" ] || [ -z "$GITEA_PASS" ]; then
    echo "ERROR: could not parse credentials for $HOST" >&2
    exit 1
fi

RESP_FILE=$(mktemp)
trap 'rm -f "$RESP_FILE"' EXIT

echo "Creating repository $REPO_NAME on $HOST (owner: $GITEA_USER) ..."
STATUS=$(curl -s -o "$RESP_FILE" -w "%{http_code}" \
    -X POST "https://$HOST/api/v1/user/repos" \
    -u "$GITEA_USER:$GITEA_PASS" \
    -H "Content-Type: application/json" \
    -d "{\"name\":\"$REPO_NAME\",\"description\":\"$DESCRIPTION\",\"private\":false,\"auto_init\":false}")

case "$STATUS" in
    201) echo "Repository created." ;;
    409) echo "Repository already exists - continuing." ;;
    401) echo "ERROR: authentication failed (HTTP 401). Is 2FA enabled or password wrong? A token may be needed." >&2; exit 1 ;;
    403) echo "ERROR: forbidden (HTTP 403)." >&2; exit 1 ;;
    *)   echo "ERROR: unexpected HTTP status $STATUS" >&2; cat "$RESP_FILE" >&2; exit 1 ;;
esac

# The git-credentials username may be an email; the repo URL needs the
# actual Gitea account name. Resolve it via the /user endpoint.
OWNER=$(curl -s -u "$GITEA_USER:$GITEA_PASS" "https://$HOST/api/v1/user" \
    | python3 -c 'import json,sys; print(json.load(sys.stdin)["login"])')

echo "Resolved Gitea owner: $OWNER"

REMOTE_URL="https://$HOST/$OWNER/$REPO_NAME.git"

cd "$(dirname "$0")/.."

if [ ! -d .git ]; then
    git init -b main
fi

if git remote get-url origin >/dev/null 2>&1; then
    git remote set-url origin "$REMOTE_URL"
else
    git remote add origin "$REMOTE_URL"
fi

if ! git diff --cached --quiet 2>/dev/null || [ -n "$(git status --porcelain)" ]; then
    git add -A
    git commit -m "Initial project structure: documentation, server concept, agents instructions"
fi

echo "Pushing to $REMOTE_URL (branch: main) ..."
git push -u origin main

echo "Done."
