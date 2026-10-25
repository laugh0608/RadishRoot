OUTPUT_FORMAT(elf64-x86-64)
OUTPUT_ARCH(i386:x86-64)
ENTRY(_start)

PHDRS {
  text PT_LOAD FLAGS(5);
  rodata PT_LOAD FLAGS(4);
  data PT_LOAD FLAGS(6);
}

SECTIONS {
  . = 0xffffffff80000000;
  .text ALIGN(4096) : { *(.text .text.*) } :text
  .rodata ALIGN(4096) : { *(.rodata .rodata.*) } :rodata
  .data ALIGN(4096) : { *(.data .data.*) } :data
  .bss ALIGN(4096) (NOLOAD) : { *(.bss .bss.*) *(COMMON) } :data
  /DISCARD/ : { *(.eh_frame .eh_frame_hdr .comment .note.GNU-stack) }
  .debug_abbrev 0 : { *(.debug_abbrev) }
  .debug_info 0 : { *(.debug_info) }
  .debug_aranges 0 : { *(.debug_aranges) }
  .debug_ranges 0 : { *(.debug_ranges) }
  .debug_str 0 : { *(.debug_str) }
  .debug_line 0 : { *(.debug_line) }
  .debug_loc 0 : { *(.debug_loc) }
  .debug_frame 0 : { *(.debug_frame) }
  .symtab 0 : { *(.symtab) }
  .strtab 0 : { *(.strtab) }
  .shstrtab 0 : { *(.shstrtab) }
  ASSERT(_start >= ADDR(.text) && _start < ADDR(.text) + SIZEOF(.text), "entry outside text")
  ASSERT(ADDR(.text) == 0xffffffff80000000, "unexpected text base")
}
