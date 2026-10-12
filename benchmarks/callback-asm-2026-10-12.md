# Callback assembly inspection — 2026-10-12

Source: `0d07520a85c895a4b36ccf3c3e73954b70877e20`. Host r5900xt;
Rust 1.88.0; x86-64 default target, no target-cpu=native. `cargo asm` uses
release optimization. TMPDIR is `/home/lilith/tmp`; CARGO_INCREMENTAL=0.
The new `just checkpoint-asm` recipe makes these inspections reproducible.

Commands, each under `run-heavy --mem 16G --`:

```sh
just checkpoint-asm
just checkpoint-asm std,callback
just checkpoint-asm std,callback '<how_far_along::callback::FnPulse as enough::Stop>::check'
```

## Ordinary trees

The first two outputs match exactly after excluding Cargo and run-heavy logs:
ten assembly instructions, no conditional branch, and one indirect tail jump
to the existing erased cancellation policy. Calling the tree through `&dyn Pulse`
can additionally use the ordinary Pulse vtable at the caller; this refactor does
not introduce that dispatch. Policy selection is private monomorphization, not
an enum or runtime choice between callback and cancellation modes.

```asm
<how_far_along::pulse::PulseTree as enough::Stop>::check:
	mov rax, qword ptr [rdi]
	mov rcx, qword ptr [rax + 32]
	mov rax, qword ptr [rax + 40]
	mov rdx, qword ptr [rax + 16]
	mov rax, qword ptr [rax + 24]
	dec rdx
	and rdx, -16
	lea rdi, [rcx + rdx]
	add rdi, 16
	jmp rax
```

## Callback-backed trees

Dispatcher logic is inlined into `FnPulse::check`. It loads the shared stop latch,
checks whether the root or current phase finished, and calls the user's erased
callback once when allowed. A callback stop uses `lock cmpxchg` to latch the first
reason, and a final latch load observes concurrent stops. There is no additional
indirect policy call or runtime mode switch. Those callback-specific operations
are real work, confined to callback-backed pulses.

```asm
<how_far_along::callback::FnPulse as enough::Stop>::check:
	push rbx
	sub rsp, 16
	mov rcx, qword ptr [rdi]
	mov rbx, qword ptr [rcx + 32]
	movzx eax, byte ptr [rbx + 40]
	test al, al
	je .LBB137_1
	cmp al, 2
.LBB137_7:
	sete al
	jmp .LBB137_8
.LBB137_1:
	mov rax, qword ptr [rbx + 32]
	movzx edx, byte ptr [rax + 146]
	mov al, 2
	cmp dl, 2
	je .LBB137_8
	add rcx, 24
	mov rdx, qword ptr [rcx]
	movzx edx, byte ptr [rdx + 146]
	cmp dl, 2
	je .LBB137_8
	lea rax, [rbx + 32]
	mov qword ptr [rsp], rax
	mov qword ptr [rsp + 8], rcx
	mov rdi, qword ptr [rbx + 16]
	mov rax, qword ptr [rbx + 24]
	mov rsi, rsp
	call qword ptr [rax + 40]
	cmp al, 2
	je .LBB137_5
	inc al
	mov ecx, eax
	xor eax, eax
	lock cmpxchg	byte ptr [rbx + 40], cl
.LBB137_5:
	movzx ecx, byte ptr [rbx + 40]
	test cl, cl
	mov al, 2
	je .LBB137_8
	cmp cl, 2
	jmp .LBB137_7
.LBB137_8:
	add rsp, 16
	pop rbx
	ret
```

## Compile time and measurement limits

The [five-sample compile measurements](callback-isolation-2026-10-12.md) compare
tracker defaults with `callback` enabled, on the same host and Rust 1.88.0:

| rustc invocation | Default | With callback | Difference |
| --- | ---: | ---: | ---: |
| Check | 96 ms | 102 ms | +6 ms |
| Debug | 176 ms | 185 ms | +9 ms |
| Release | 281 ms | 294 ms | +13 ms |

These are warm-file compiler invocations with dependencies already built, not
end-to-end cold Cargo builds. The callback feature's absolute compilation cost
is small in this measurement. It remains outside the existing default IR budget
(24,376 lines with callback, 22,000 allowed); that is a code-size guard, not a
measured compile-time threshold. The default build measures 20,397 lines.

The previous runtime benchmark attempts did not provide a reliable latency
comparison; no cycle count or nanosecond overhead is claimed here. The assembly
establishes that enabling the feature adds no ordinary checkpoint instructions.

Resource logs for these assembly builds (not runtime pulse memory measurements):

```text
run-heavy: done rc=0 1s | peak-RSS 0.03GiB | min-avail 53460MiB | peak-load 0.07
run-heavy: done rc=0 1s | peak-RSS 0.03GiB | min-avail 53446MiB | peak-load 0.06
run-heavy: done rc=0 1s | peak-RSS 0.03GiB | min-avail 53415MiB | peak-load 0.06
```
