#!/bin/bash
# scripts/init-ansible-workspace.sh — Bootstrap /opt/ansible on target machine
# =============================================================================
# Creates a demo Ansible workspace on the ansible-prod sandbox host.
# Requires SSH access to the target machine.
#
# Usage:
#   # From local machine with SSH key:
#   SSH_KEY=~/.ssh/id_rsa ./scripts/init-ansible-workspace.sh
#
#   # Specify custom host/user:
#   HOST=192.168.2.106 USER=root SSH_KEY=~/.ssh/id_ed25519 \
#     ./scripts/init-ansible-workspace.sh
#
#   # From CI / after deploying the ansible-ssh-key secret:
#   kubectl get secret ansible-ssh-key -n vol-agent-system \
#     -o jsonpath='{.data.id_ed25519}' | base64 -d > /tmp/ansible-key
#   chmod 600 /tmp/ansible-key
#   SSH_KEY=/tmp/ansible-key ./scripts/init-ansible-workspace.sh
# =============================================================================
set -euo pipefail

HOST="${HOST:-192.168.2.106}"
PORT="${PORT:-22}"
USER="${USER:-root}"
SSH_KEY="${SSH_KEY:-}"
WORKSPACE="${WORKSPACE:-/opt/ansible}"

if [ -z "$SSH_KEY" ] || [ ! -f "$SSH_KEY" ]; then
    echo "ERROR: SSH_KEY not set or file not found: ${SSH_KEY:-<unset>}"
    echo "Usage: SSH_KEY=/path/to/key $0"
    exit 1
fi

SSH_OPTS="-o StrictHostKeyChecking=accept-new -i $SSH_KEY -p $PORT"
SSH="ssh $SSH_OPTS ${USER}@${HOST}"
SCP="scp $SSH_OPTS"

echo "============================================"
echo "  Init Ansible Workspace"
echo "  Target: ${USER}@${HOST}:${PORT}"
echo "  Workspace: ${WORKSPACE}"
echo "============================================"
echo ""

# ── Verify SSH connectivity ───────────────────────────────────────────────
echo "[0/5] Testing SSH connectivity..."
if ! $SSH 'echo OK' 2>/dev/null; then
    echo "ERROR: Cannot SSH to ${USER}@${HOST}:${PORT}"
    echo "Check: key permissions, network connectivity, SSH daemon"
    exit 1
fi

# ── Create directory structure ─────────────────────────────────────────────
echo "[1/5] Creating directories..."
$SSH "mkdir -p ${WORKSPACE}/{inventories/{production,staging},playbooks,roles,group_vars,host_vars,files,templates,.cache/facts}"

# ── Ensure ansible is installed ────────────────────────────────────────────
echo "[2/5] Checking ansible installation..."
$SSH 'which ansible 2>/dev/null' || {
    echo "  Installing ansible..."
    $SSH 'apt-get update -qq && apt-get install -y -qq ansible' 2>/dev/null || \
    $SSH 'apk add --no-cache ansible' 2>/dev/null || \
    $SSH 'pip3 install ansible' 2>/dev/null || \
    echo "  WARNING: ansible not installed — install manually on target"
}

# ── ansible.cfg ───────────────────────────────────────────────────────────
echo "[3/5] Writing ansible.cfg..."
$SSH "cat > ${WORKSPACE}/ansible.cfg << 'ANSCFG'
[defaults]
inventory      = ${WORKSPACE}/inventories/production/hosts.yml
host_key_checking = False
retry_files_enabled = False
gathering        = implicit
fact_caching     = jsonfile
fact_caching_connection = ${WORKSPACE}/.cache/facts
fact_caching_timeout = 86400
stdout_callback  = yaml
callback_whitelist = profile_tasks
forks            = 5
timeout          = 60
remote_user      = root

[ssh_connection]
pipelining = True
control_path = /tmp/ansible-%%h-%%p-%%r
control_path_dir = /tmp
ANSCFG"

# ── Inventory ─────────────────────────────────────────────────────────────
echo "[4/5] Writing inventory..."
$SSH "cat > ${WORKSPACE}/inventories/production/hosts.yml << 'YML'
all:
  hosts:
    localhost:
      ansible_connection: local
    target-host:
      ansible_host: ${HOST}
      ansible_user: ${USER}
  children:
    webservers:
      hosts:
        target-host:
    dev:
      hosts:
        localhost:
YML"

# ── Demo playbooks ─────────────────────────────────────────────────────────
echo "[5/5] Writing demo playbooks..."

$SSH "cat > ${WORKSPACE}/playbooks/ping.yml << 'YML'
---
- name: Demo — ping all hosts
  hosts: all
  gather_facts: false
  tasks:
    - name: Ping
      ansible.builtin.ping:

    - name: Show uptime
      ansible.builtin.command: uptime
      changed_when: false
YML"

$SSH "cat > ${WORKSPACE}/playbooks/system-info.yml << 'YML'
---
- name: Gather system information
  hosts: all
  gather_facts: true
  tasks:
    - name: OS version
      ansible.builtin.debug:
        msg: \"{{ ansible_distribution }} {{ ansible_distribution_version }}\"

    - name: Disk usage
      ansible.builtin.shell: df -h /
      changed_when: false

    - name: Memory
      ansible.builtin.debug:
        msg: \"{{ ansible_memtotal_mb }} MB total, {{ ansible_memfree_mb }} MB free\"
YML"

# ── Verify ─────────────────────────────────────────────────────────────────
echo ""
echo "============================================"
echo "  Initialization Complete!"
echo "============================================"
echo ""
$SSH "find ${WORKSPACE} -type f -o -type d | sort" 2>/dev/null || \
  $SSH "ls -laR ${WORKSPACE}"
echo ""
echo "Test with:"
echo "  ssh -i $SSH_KEY ${USER}@${HOST} 'cd ${WORKSPACE} && ansible all -m ping'"
echo ""
echo "Or via the cli-tools-mcp sandbox (once deployed):"
echo "  # The agent will use sandbox_ref=ansible-prod with cwd=/opt/ansible"
