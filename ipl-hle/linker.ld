ENTRY(_start)

PHDRS
{
  text_eh PT_LOAD FLAGS(5);
  text_prog PT_LOAD FLAGS(5);
  data PT_LOAD FLAGS(6);
  bss  PT_LOAD;
}

SECTIONS {
  . = 0x80000C00;
  .eh.syscall : {
    KEEP(*(.text.eh.syscall))
  } : text_eh

  . = 0x81300000;
  .text : {
    *(.text.init)
    *(.text)
    *(.text.*)
  } : text_prog

  .data : { * (.data .data.*) } : data
  .rodata : { * (.rodata .rodata.*) } : data
  .bss : { * (.bss .bss.*) } : bss
}
