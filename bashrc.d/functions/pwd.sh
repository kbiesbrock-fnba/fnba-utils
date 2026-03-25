# pwd that also copies path to clipboard
pwd() {
  builtin pwd | clip.exe
  builtin pwd
}
