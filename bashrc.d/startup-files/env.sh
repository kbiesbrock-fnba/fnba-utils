# PATH, JAVA_HOME, Node CA certs

# Source every *.env in ~/.fnba/ (chmod-600 location for secrets).
# set -a auto-exports every assignment so child processes inherit them.
# Files load in alphabetical order; later files override earlier ones on key collision.
if [ -d "$HOME/.fnba" ]; then
  set -a
  for f in "$HOME"/.fnba/*.env; do
    [ -r "$f" ] && source "$f"
  done
  set +a
  unset f
fi

export NODE_TLS_REJECT_UNAUTHORIZED=0
export NODE_EXTRA_CA_CERTS="$HOME/corporate-ca.pem"
export PATH="$HOME/.local/bin:$PATH"
export JAVA_HOME="$HOME/.sdkman/candidates/java/current"
