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
    lea rax, [rip + .Lstr_0]
    mov [rbp - 16], rax
    mov rax, 25
    mov [rbp - 32], rax
    lea rax, [rip + .Lstr_1]
    mov [rbp - 48], rax
    lea rsi, [rip + .Lwstr_2]
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
    mov rax, [rbp - 16]
    mov rdi, rax
    xor rax, rax
.Lstrlen_loop_3:
    cmp byte ptr [rdi + rax], 0
    je .Lstrlen_done_4
    inc rax
    jmp .Lstrlen_loop_3
.Lstrlen_done_4:
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
    lea rsi, [rip + .Lwstr_6]
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
    mov rax, [rbp - 32]
    sub rsp, 32
    // itoa: convertir rax a ASCII en [rsp]
    mov rcx, rax
    mov rax, rcx
    mov rbx, 10
    lea rdi, [rsp + 20]
    mov byte ptr [rdi], 0
    test rax, rax
    jns .Litoa_pos_7
    neg rax
.Litoa_pos_7:
.Litoa_loop_8:
    dec rdi
    xor rdx, rdx
    div rbx
    add dl, '0'
    mov [rdi], dl
    test rax, rax
    jnz .Litoa_loop_8
    test rcx, rcx
    jns .Litoa_done_9
    dec rdi
    mov byte ptr [rdi], '-'
.Litoa_done_9:
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
    lea rsi, [rip + .Lnl_11]
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
    lea rsi, [rip + .Lwstr_12]
    mov rdx, 8
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
    mov rax, [rbp - 48]
    mov rdi, rax
    xor rax, rax
.Lstrlen_loop_13:
    cmp byte ptr [rdi + rax], 0
    je .Lstrlen_done_14
    inc rax
    jmp .Lstrlen_loop_13
.Lstrlen_done_14:
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
    mov rax, 26
    mov [rbp - 32], rax
    lea rsi, [rip + .Lwstr_16]
    mov rdx, 12
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
    mov rax, [rbp - 32]
    sub rsp, 32
    // itoa: convertir rax a ASCII en [rsp]
    mov rcx, rax
    mov rax, rcx
    mov rbx, 10
    lea rdi, [rsp + 20]
    mov byte ptr [rdi], 0
    test rax, rax
    jns .Litoa_pos_17
    neg rax
.Litoa_pos_17:
.Litoa_loop_18:
    dec rdi
    xor rdx, rdx
    div rbx
    add dl, '0'
    mov [rdi], dl
    test rax, rax
    jnz .Litoa_loop_18
    test rcx, rcx
    jns .Litoa_done_19
    dec rdi
    mov byte ptr [rdi], '-'
.Litoa_done_19:
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
    lea rsi, [rip + .Lwstr_20]
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
    lea rsi, [rip + .Lnl_21]
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
.Lstr_0:
    .asciz "Ana"
.Lstr_1:
    .asciz "Argentina"
.Lwstr_2:
    .asciz "Hola, soy "
.Lnl_5:
    .asciz "\n"
.Lwstr_6:
    .asciz "Tengo "
.Lwstr_10:
    .asciz " años"
.Lnl_11:
    .asciz "\n"
.Lwstr_12:
    .asciz "Vivo en "
.Lnl_15:
    .asciz "\n"
.Lwstr_16:
    .asciz "Ahora tengo "
.Lwstr_20:
    .asciz " años"
.Lnl_21:
    .asciz "\n"
