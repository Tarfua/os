/* Identity-mapped load address for bootloader 0.11.
 * PHDRS forces exactly two PT_LOAD segments (text+RX, data+RW), each starting
 * on a 4K boundary, so the bootloader never maps the same page twice.
 * LLD syntax: :phdr comes after the closing brace. */
ENTRY(_start)

PHDRS
{
    text PT_LOAD FLAGS(5);
    data PT_LOAD FLAGS(6);
}

SECTIONS
{
    . = 0x1000000;

    .text : ALIGN(4096) {
        *(.text .text.*)
    } :text

    .rodata : ALIGN(4096) {
        *(.rodata .rodata.*)
    } :text

    . = ALIGN(4096);
    .data : {
        *(.data .data.*)
        *(.got .got.*)
        *(.got.plt)
    } :data

    .bss : ALIGN(4096) {
        *(COMMON)
        *(.bss .bss.*)
        . = ALIGN(16);
        _stack_bottom = .;
        . += 0x10000;
        _stack_top = .;
    } :data

    /DISCARD/ : {
        *(.eh_frame)
        *(.note.GNU-stack)
    }

    _kernel_end = .;
}
