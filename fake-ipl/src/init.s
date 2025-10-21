.extern main

.section .data.stack
.space 0x1000
stack:
.space 0x1000

.section .text.init
.global _start
_start:
  # Setup stack pointer
  lis %r1, stack@ha
  ori %r1, %r1, stack@l

  # Call main
  b main
