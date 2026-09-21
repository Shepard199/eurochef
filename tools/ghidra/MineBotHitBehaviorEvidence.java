import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.mem.Memory;

public class MineBotHitBehaviorEvidence extends GhidraScript {
    private void dump(long raw, DecompInterface d) throws Exception {
        Address a = toAddr(raw);
        println(String.format("\n=== 0x%08X ===", raw));
        Function f = getFunctionContaining(a);
        if (f == null) { println("NO_FUNCTION"); return; }
        println("FUNCTION=" + f.getName() + " ENTRY=" + f.getEntryPoint());
        DecompileResults r = d.decompileFunction(f, 60, monitor);
        if (r.decompileCompleted() && r.getDecompiledFunction() != null) {
            println(r.getDecompiledFunction().getC());
        }
    }

    @Override public void run() throws Exception {
        DecompInterface d = new DecompInterface();
        d.openProgram(currentProgram);
        Memory memory = currentProgram.getMemory();
        long[] behaviorVtables = new long[]{
            0x005E732CL, 0x005E25D0L, 0x005E6EF0L, 0x005E2140L, 0x005E23D0L,
            0x005E240CL, 0x005E2448L, 0x005E2484L, 0x005E24C0L, 0x005E2100L
        };
        for (long vt : behaviorVtables) {
            for (long off : new long[]{0x08L, 0x10L, 0x14L, 0x18L, 0x20L, 0x30L, 0x34L, 0x38L, 0x3CL, 0x40L}) {
                long target = memory.getInt(toAddr(vt + off)) & 0xffffffffL;
                println(String.format("BEHAVIOR_VTABLE 0x%08X +0x%02X -> 0x%08X", vt, off, target));
                dump(target, d);
            }
        }
        for (long a : new long[]{
            0x0042A670L,
            0x0045FEE0L,
            0x0046BC20L,
            0x004590A0L,
            0x0046A390L,
            0x00450CB0L,
            0x00457980L,
            0x00457E30L,
            0x004581C0L,
            0x004583A0L,
            0x004584C0L,
            0x004508A0L,
            0x00450AB0L,
            0x004510D0L,
            0x00456F00L,
            0x00456F10L
        }) dump(a, d);
        d.dispose();
    }
}
