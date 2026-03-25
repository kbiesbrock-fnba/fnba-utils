# Print and copy Windows path of cwd
wwd() {
  wslpath -w "$PWD" | clip.exe
  wslpath -w "$PWD"
}
