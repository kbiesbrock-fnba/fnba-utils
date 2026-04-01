# PATH, JAVA_HOME, Node CA certs
BASHRC_D="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
[ -f "$BASHRC_D/.env" ] && source "$BASHRC_D/.env"
unset BASHRC_D

export NODE_EXTRA_CA_CERTS="$HOME/corporate-ca.pem"
export PATH="$HOME/.local/bin:$PATH"
export JAVA_HOME="$HOME/.sdkman/candidates/java/current"
