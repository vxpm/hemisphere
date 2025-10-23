.extern main

.section .bss.stack
.space 0x800
stack:
.space 0x800

.section .text.eh.syscall
_eh_syscall:
  rfi

.section .text.init
.global _start
_start:
  # Setup stack pointer
  lis %r1, stack@ha
  ori %r1, %r1, stack@l

  # Call main
  b main
