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

main:
    push rbp
    mov rbp, rsp
    sub rsp, 64
    lea rsi, [rip + .Lwstr_0]
    mov rdx, 14
    sub rsp, 48
    mov rcx, -11
    call GetStdHandle
    mov rcx, rax
    mov rdx, rsi
    mov r8, rdx
    lea r9, [rsp + 40]
    mov qword ptr [rsp + 32], 0
    call WriteFile
    add rsp, 48
    lea rsi, [rip + .Lnl_1]
    mov rdx, 1
    sub rsp, 48
    mov rcx, -11
    call GetStdHandle
    mov rcx, rax
    mov rdx, rsi
    mov r8, rdx
    lea r9, [rsp + 40]
    mov qword ptr [rsp + 32], 0
    call WriteFile
    add rsp, 48
    lea rsi, [rip + .Lwstr_2]
    mov rdx, 19
    sub rsp, 48
    mov rcx, -11
    call GetStdHandle
    mov rcx, rax
    mov rdx, rsi
    mov r8, rdx
    lea r9, [rsp + 40]
    mov qword ptr [rsp + 32], 0
    call WriteFile
    add rsp, 48
    lea rsi, [rip + .Lnl_3]
    mov rdx, 1
    sub rsp, 48
    mov rcx, -11
    call GetStdHandle
    mov rcx, rax
    mov rdx, rsi
    mov r8, rdx
    lea r9, [rsp + 40]
    mov qword ptr [rsp + 32], 0
    call WriteFile
    add rsp, 48
    lea rsi, [rip + .Lwstr_4]
    mov rdx, 2
    sub rsp, 48
    mov rcx, -11
    call GetStdHandle
    mov rcx, rax
    mov rdx, rsi
    mov r8, rdx
    lea r9, [rsp + 40]
    mov qword ptr [rsp + 32], 0
    call WriteFile
    add rsp, 48
    lea rsi, [rip + .Lnl_5]
    mov rdx, 1
    sub rsp, 48
    mov rcx, -11
    call GetStdHandle
    mov rcx, rax
    mov rdx, rsi
    mov r8, rdx
    lea r9, [rsp + 40]
    mov qword ptr [rsp + 32], 0
    call WriteFile
    add rsp, 48

    mov eax, 0
    mov rsp, rbp
    pop rbp
    ret

.section .rdata
.Lwstr_0:
    .asciz "¡Hola, mundo!"
.Lnl_1:
    .asciz "\n"
.Lwstr_2:
    .asciz "Bienvenido a Forja!"
.Lnl_3:
    .asciz "\n"
.Lwstr_4:
    .asciz "42"
.Lnl_5:
    .asciz "\n"
