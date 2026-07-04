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
    mov rax, 10
    mov r12, rax
    mov rax, 3
    mov r13, rax
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
    mov r14, rax
    mov rax, r13
    mov rcx, rax
    mov rax, r14
    add rax, rcx
    sub rsp, 32
    // itoa: convertir rax a ASCII en [rsp]
    mov rcx, rax
    mov rax, rcx
    mov rbx, 10
    lea rdi, [rsp + 20]
    mov byte ptr [rdi], 0
    test rax, rax
    jns .Litoa_pos_1
    neg rax
.Litoa_pos_1:
.Litoa_loop_2:
    dec rdi
    xor rdx, rdx
    div rbx
    add dl, '0'
    mov [rdi], dl
    test rax, rax
    jnz .Litoa_loop_2
    test rcx, rcx
    jns .Litoa_done_3
    dec rdi
    mov byte ptr [rdi], '-'
.Litoa_done_3:
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
    lea rsi, [rip + .Lwstr_5]
    mov rdx, 7
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
    mov r14, rax
    mov rax, r13
    mov rcx, rax
    mov rax, r14
    sub rax, rcx
    sub rsp, 32
    // itoa: convertir rax a ASCII en [rsp]
    mov rcx, rax
    mov rax, rcx
    mov rbx, 10
    lea rdi, [rsp + 20]
    mov byte ptr [rdi], 0
    test rax, rax
    jns .Litoa_pos_6
    neg rax
.Litoa_pos_6:
.Litoa_loop_7:
    dec rdi
    xor rdx, rdx
    div rbx
    add dl, '0'
    mov [rdi], dl
    test rax, rax
    jnz .Litoa_loop_7
    test rcx, rcx
    jns .Litoa_done_8
    dec rdi
    mov byte ptr [rdi], '-'
.Litoa_done_8:
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
    lea rsi, [rip + .Lwstr_10]
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
    mov rax, r12
    mov r14, rax
    mov rax, r13
    mov rcx, rax
    mov rax, r14
    imul rax, rcx
    sub rsp, 32
    // itoa: convertir rax a ASCII en [rsp]
    mov rcx, rax
    mov rax, rcx
    mov rbx, 10
    lea rdi, [rsp + 20]
    mov byte ptr [rdi], 0
    test rax, rax
    jns .Litoa_pos_11
    neg rax
.Litoa_pos_11:
.Litoa_loop_12:
    dec rdi
    xor rdx, rdx
    div rbx
    add dl, '0'
    mov [rdi], dl
    test rax, rax
    jnz .Litoa_loop_12
    test rcx, rcx
    jns .Litoa_done_13
    dec rdi
    mov byte ptr [rdi], '-'
.Litoa_done_13:
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
    lea rsi, [rip + .Lwstr_15]
    mov rdx, 11
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
    mov r14, rax
    mov rax, r13
    mov rcx, rax
    mov rax, r14
    xor rdx, rdx
    idiv rcx
    sub rsp, 32
    // itoa: convertir rax a ASCII en [rsp]
    mov rcx, rax
    mov rax, rcx
    mov rbx, 10
    lea rdi, [rsp + 20]
    mov byte ptr [rdi], 0
    test rax, rax
    jns .Litoa_pos_16
    neg rax
.Litoa_pos_16:
.Litoa_loop_17:
    dec rdi
    xor rdx, rdx
    div rbx
    add dl, '0'
    mov [rdi], dl
    test rax, rax
    jnz .Litoa_loop_17
    test rcx, rcx
    jns .Litoa_done_18
    dec rdi
    mov byte ptr [rdi], '-'
.Litoa_done_18:
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
    lea rsi, [rip + .Lnl_19]
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
    lea rsi, [rip + .Lwstr_20]
    mov rdx, 11
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
    mov r14, rax
    mov rax, r13
    mov rcx, rax
    mov rax, r14
    cmp rax, rcx
    setg al
    movzx rax, al
    sub rsp, 32
    // itoa: convertir rax a ASCII en [rsp]
    mov rcx, rax
    mov rax, rcx
    mov rbx, 10
    lea rdi, [rsp + 20]
    mov byte ptr [rdi], 0
    test rax, rax
    jns .Litoa_pos_21
    neg rax
.Litoa_pos_21:
.Litoa_loop_22:
    dec rdi
    xor rdx, rdx
    div rbx
    add dl, '0'
    mov [rdi], dl
    test rax, rax
    jnz .Litoa_loop_22
    test rcx, rcx
    jns .Litoa_done_23
    dec rdi
    mov byte ptr [rdi], '-'
.Litoa_done_23:
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
    lea rsi, [rip + .Lnl_24]
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
    lea rsi, [rip + .Lwstr_25]
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
    mov rax, r12
    mov r14, rax
    mov rax, r13
    mov rcx, rax
    mov rax, r14
    cmp rax, rcx
    sete al
    movzx rax, al
    sub rsp, 32
    // itoa: convertir rax a ASCII en [rsp]
    mov rcx, rax
    mov rax, rcx
    mov rbx, 10
    lea rdi, [rsp + 20]
    mov byte ptr [rdi], 0
    test rax, rax
    jns .Litoa_pos_26
    neg rax
.Litoa_pos_26:
.Litoa_loop_27:
    dec rdi
    xor rdx, rdx
    div rbx
    add dl, '0'
    mov [rdi], dl
    test rax, rax
    jnz .Litoa_loop_27
    test rcx, rcx
    jns .Litoa_done_28
    dec rdi
    mov byte ptr [rdi], '-'
.Litoa_done_28:
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
    lea rsi, [rip + .Lnl_29]
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
    .asciz "Suma: "
.Lnl_4:
    .asciz "\n"
.Lwstr_5:
    .asciz "Resta: "
.Lnl_9:
    .asciz "\n"
.Lwstr_10:
    .asciz "Multiplicación: "
.Lnl_14:
    .asciz "\n"
.Lwstr_15:
    .asciz "División: "
.Lnl_19:
    .asciz "\n"
.Lwstr_20:
    .asciz "¿10 > 3?: "
.Lnl_24:
    .asciz "\n"
.Lwstr_25:
    .asciz "¿10 == 3?: "
.Lnl_29:
    .asciz "\n"
