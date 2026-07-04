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

saludar:
    push r12
    push r13
    push r14
    push r15
    push rbp
    mov rbp, rsp
    sub rsp, 48
    mov r12, rcx

    lea rsi, [rip + .Lwstr_0]
    mov rdx, 6
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
    mov rax, r12
    mov rdi, rax
    xor rax, rax
.Lstrlen_loop_1:
    cmp byte ptr [rdi + rax], 0
    je .Lstrlen_done_2
    inc rax
    jmp .Lstrlen_loop_1
.Lstrlen_done_2:
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
    lea rsi, [rip + .Lwstr_3]
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
    lea rsi, [rip + .Lnl_4]
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

    mov rsp, rbp
    pop rbp
    pop r15
    pop r14
    pop r13
    pop r12
    ret

despedir:
    push r12
    push r13
    push r14
    push r15
    push rbp
    mov rbp, rsp
    sub rsp, 48
    mov r12, rcx

    lea rsi, [rip + .Lwstr_5]
    mov rdx, 5
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
    mov rax, r12
    mov rdi, rax
    xor rax, rax
.Lstrlen_loop_6:
    cmp byte ptr [rdi + rax], 0
    je .Lstrlen_done_7
    inc rax
    jmp .Lstrlen_loop_6
.Lstrlen_done_7:
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
    lea rsi, [rip + .Lnl_8]
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

    mov rsp, rbp
    pop rbp
    pop r15
    pop r14
    pop r13
    pop r12
    ret

main:
    push r12
    push r13
    push r14
    push r15
    push rbp
    mov rbp, rsp
    sub rsp, 64
    // inline saludar
    lea rax, [rip + .Lstr_9]
    mov [rbp - 8], rax
    lea rsi, [rip + .Lwstr_10]
    mov rdx, 6
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
    mov rax, [rbp - 8]
    mov rdi, rax
    xor rax, rax
.Lstrlen_loop_11:
    cmp byte ptr [rdi + rax], 0
    je .Lstrlen_done_12
    inc rax
    jmp .Lstrlen_loop_11
.Lstrlen_done_12:
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
    lea rsi, [rip + .Lwstr_13]
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
    lea rsi, [rip + .Lnl_14]
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
    // inline saludar
    lea rax, [rip + .Lstr_15]
    mov [rbp - 8], rax
    lea rsi, [rip + .Lwstr_16]
    mov rdx, 6
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
    mov rax, [rbp - 8]
    mov rdi, rax
    xor rax, rax
.Lstrlen_loop_17:
    cmp byte ptr [rdi + rax], 0
    je .Lstrlen_done_18
    inc rax
    jmp .Lstrlen_loop_17
.Lstrlen_done_18:
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
    lea rsi, [rip + .Lwstr_19]
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
    lea rsi, [rip + .Lnl_20]
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
    // inline despedir
    lea rax, [rip + .Lstr_21]
    mov [rbp - 8], rax
    lea rsi, [rip + .Lwstr_22]
    mov rdx, 5
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
    mov rax, [rbp - 8]
    mov rdi, rax
    xor rax, rax
.Lstrlen_loop_23:
    cmp byte ptr [rdi + rax], 0
    je .Lstrlen_done_24
    inc rax
    jmp .Lstrlen_loop_23
.Lstrlen_done_24:
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
    lea rsi, [rip + .Lnl_25]
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
    // inline despedir
    lea rax, [rip + .Lstr_26]
    mov [rbp - 8], rax
    lea rsi, [rip + .Lwstr_27]
    mov rdx, 5
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
    mov rax, [rbp - 8]
    mov rdi, rax
    xor rax, rax
.Lstrlen_loop_28:
    cmp byte ptr [rdi + rax], 0
    je .Lstrlen_done_29
    inc rax
    jmp .Lstrlen_loop_28
.Lstrlen_done_29:
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
    lea rsi, [rip + .Lnl_30]
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
    pop r15
    pop r14
    pop r13
    pop r12
    ret

.section .rdata
.Lwstr_0:
    .asciz "Hola, "
.Lwstr_3:
    .asciz "!"
.Lnl_4:
    .asciz "\n"
.Lwstr_5:
    .asciz "Chao "
.Lnl_8:
    .asciz "\n"
.Lstr_9:
    .asciz "Ana"
.Lwstr_10:
    .asciz "Hola, "
.Lwstr_13:
    .asciz "!"
.Lnl_14:
    .asciz "\n"
.Lstr_15:
    .asciz "Pedro"
.Lwstr_16:
    .asciz "Hola, "
.Lwstr_19:
    .asciz "!"
.Lnl_20:
    .asciz "\n"
.Lstr_21:
    .asciz "Ana"
.Lwstr_22:
    .asciz "Chao "
.Lnl_25:
    .asciz "\n"
.Lstr_26:
    .asciz "Pedro"
.Lwstr_27:
    .asciz "Chao "
.Lnl_30:
    .asciz "\n"
