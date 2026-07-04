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
    push r12
    push r13
    push r14
    push r15
    push rbp
    mov rbp, rsp
    sub rsp, 64
    mov rax, 18
    mov r12, rax
    mov rax, r12
    mov r13, rax
    mov rax, 18
    mov rcx, rax
    mov rax, r13
    cmp rax, rcx
    setge al
    movzx rax, al
    test rax, rax
    jz .Lelse_0
    lea rsi, [rip + .Lwstr_2]
    mov rdx, 17
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
    lea rsi, [rip + .Lnl_3]
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
    jmp .Lendif_1
.Lelse_0:
    lea rsi, [rip + .Lwstr_4]
    mov rdx, 17
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
    lea rsi, [rip + .Lnl_5]
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
.Lendif_1:
    mov rax, 85
    mov r13, rax
    mov rax, r13
    mov r14, rax
    mov rax, 90
    mov rcx, rax
    mov rax, r14
    cmp rax, rcx
    setge al
    movzx rax, al
    test rax, rax
    jz .Lelse_6
    lea rsi, [rip + .Lwstr_8]
    mov rdx, 10
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
    lea rsi, [rip + .Lnl_9]
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
    jmp .Lendif_7
.Lelse_6:
    mov rax, r13
    mov r14, rax
    mov rax, 70
    mov rcx, rax
    mov rax, r14
    cmp rax, rcx
    setge al
    movzx rax, al
    test rax, rax
    jz .Lelse_10
    lea rsi, [rip + .Lwstr_12]
    mov rdx, 13
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
    lea rsi, [rip + .Lnl_13]
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
    jmp .Lendif_11
.Lelse_10:
    lea rsi, [rip + .Lwstr_14]
    mov rdx, 18
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
    lea rsi, [rip + .Lnl_15]
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
.Lendif_11:
.Lendif_7:

    mov eax, 0
    mov rsp, rbp
    pop rbp
    pop r15
    pop r14
    pop r13
    pop r12
    ret

.section .rdata
.Lwstr_2:
    .asciz "Sos mayor de edad"
.Lnl_3:
    .asciz "\n"
.Lwstr_4:
    .asciz "Sos menor de edad"
.Lnl_5:
    .asciz "\n"
.Lwstr_8:
    .asciz "Excelente!"
.Lnl_9:
    .asciz "\n"
.Lwstr_12:
    .asciz "Buen trabajo!"
.Lnl_13:
    .asciz "\n"
.Lwstr_14:
    .asciz "Seguí estudiando!"
.Lnl_15:
    .asciz "\n"
