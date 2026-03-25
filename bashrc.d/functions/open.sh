# Open file manager from terminal
open() {
  nautilus "${1:-.}" > /dev/null 2>&1 &
}
