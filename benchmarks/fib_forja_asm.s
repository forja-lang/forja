// Código assembly generado por Forja (fa) — target: x86-64 Windows
// Compilar: gcc -O2 -o programa este_archivo.s

.intel_syntax noprefix

.section .data

.section .rdata
fmt_int:
    .asciz "%lld"
fmt_str:
    .asciz "%s"
fmt_float:
    .asciz "%f"
fmt_bool_true:
    .asciz "verdadero"
fmt_bool_false:
    .asciz "falso"
fmt_newline:
    .asciz "\n"

.section .text
.globl main
.extern printf
.extern exit
.extern malloc
.extern free
.extern GetStdHandle
.extern WriteFile

forja_print_int:
    push rbp
    mov rbp, rsp
    sub rsp, 32
    mov rdx, rax
    lea rcx, [rip + fmt_int]
    call printf
    mov rsp, rbp
    pop rbp
    ret

forja_print_str:
    push rbp
    mov rbp, rsp
    sub rsp, 32
    mov rdx, rax
    lea rcx, [rip + fmt_str]
    call printf
    mov rsp, rbp
    pop rbp
    ret

forja_print_float:
    push rbp
    mov rbp, rsp
    sub rsp, 32
    lea rcx, [rip + fmt_float]
    call printf
    mov rsp, rbp
    pop rbp
    ret

forja_print_bool:
    push rbp
    mov rbp, rsp
    sub rsp, 32
    test rax, rax
    jz .Lprint_false
    lea rdx, [rip + fmt_bool_true]
    jmp .Lprint_bool_end
.Lprint_false:
    lea rdx, [rip + fmt_bool_false]
.Lprint_bool_end:
    lea rcx, [rip + fmt_str]
    call printf
    mov rsp, rbp
    pop rbp
    ret

forja_print_newline:
    push rbp
    mov rbp, rsp
    sub rsp, 32
    lea rcx, [rip + fmt_newline]
    call printf
    mov rsp, rbp
    pop rbp
    ret

fibonacci:
    push rbp
    mov rbp, rsp
    sub rsp, 64
    mov [rbp - 8], rcx

    mov rax, [rbp - 8]
    mov rbx, rax
    mov rax, 1
    mov rcx, rax
    mov rax, rbx
    cmp rax, rcx
    setle al
    movzx rax, al
    test rax, rax
    jz .Lelse_0
    mov rax, [rbp - 8]
    mov rsp, rbp
    pop rbp
    ret
    jmp .Lendif_1
.Lelse_0:
.Lendif_1:
    mov rax, 0
    mov [rbp - 24], rax
    mov rax, 1
    mov [rbp - 40], rax
    mov rax, 2
    mov [rbp - 56], rax
.Lwhile_start_2:
    mov rax, [rbp - 56]
    mov rbx, rax
    mov rax, [rbp - 8]
    mov rcx, rax
    mov rax, rbx
    cmp rax, rcx
    setle al
    movzx rax, al
    test rax, rax
    jz .Lwhile_end_3
    mov rax, [rbp - 24]
    mov rbx, rax
    mov rax, [rbp - 40]
    mov rcx, rax
    mov rax, rbx
    add rax, rcx
    mov [rbp - 72], rax
    mov rax, [rbp - 40]
    mov [rbp - 24], rax
    mov rax, [rbp - 72]
    mov [rbp - 40], rax
    mov rax, [rbp - 56]
    mov rbx, rax
    mov rax, 1
    mov rcx, rax
    mov rax, rbx
    add rax, rcx
    mov [rbp - 56], rax
    jmp .Lwhile_start_2
.Lwhile_end_3:
    mov rax, [rbp - 40]
    mov rsp, rbp
    pop rbp
    ret

    mov rsp, rbp
    pop rbp
    ret

main:
    push rbp
    mov rbp, rsp
    sub rsp, 64
    mov rax, 40
    sub rsp, 32
    call fibonacci
    add rsp, 32
    mov [rbp - 16], rax
    mov rax, [rbp - 16]
    sub rsp, 32
    // itoa: convertir rax a ASCII en [rsp]
    mov rcx, rax
    mov rax, rcx
    mov rbx, 10
    lea rdi, [rsp + 20]
    mov byte ptr [rdi], 0
    test rax, rax
    jns .Litoa_pos_4
    neg rax
.Litoa_pos_4:
.Litoa_loop_5:
    dec rdi
    xor rdx, rdx
    div rbx
    add dl, '0'
    mov [rdi], dl
    test rax, rax
    jnz .Litoa_loop_5
    test rcx, rcx
    jns .Litoa_done_6
    dec rdi
    mov byte ptr [rdi], '-'
.Litoa_done_6:
    // rax = longitud, rdi = ptr string
    lea rax, [rsp + 20]
    sub rax, rdi
    mov rsi, rdi
    mov rdx, rax
    sub rsp, 48
    mov rcx, -11
    call GetStdHandle
    mov rcx, rax
    mov r8, rdx
    mov rdx, rsi
    lea r9, [rsp + 40]
    mov qword ptr [rsp + 32], 0
    call WriteFile
    add rsp, 48
    add rsp, 32
    lea rsi, [rip + .Lnl_7]
    mov rdx, 1
    sub rsp, 48
    mov rcx, -11
    call GetStdHandle
    mov rcx, rax
    mov r8, rdx
    mov rdx, rsi
    lea r9, [rsp + 40]
    mov qword ptr [rsp + 32], 0
    call WriteFile
    add rsp, 48

    mov eax, 0
    mov rsp, rbp
    pop rbp
    ret

.section .rdata
.Lnl_7:
    .asciz "\n"
