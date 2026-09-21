import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
import java.util.LinkedHashSet;
import java.util.Set;

public class MineBotAttackNodeEvidence extends GhidraScript {
    private long ptr(long address) throws Exception {
        return Integer.toUnsignedLong(getInt(toAddr(address)));
    }

    private void dump(long raw, DecompInterface d) throws Exception {
        Function f = getFunctionContaining(toAddr(raw));
        println(String.format("\n=== 0x%08X ===", raw));
        if (f == null) { println("NO FUNCTION"); return; }
        println("FUNCTION=" + f.getName() + " ENTRY=" + f.getEntryPoint());
        DecompileResults r = d.decompileFunction(f, 60, monitor);
        if (r.decompileCompleted() && r.getDecompiledFunction() != null) {
            println(r.getDecompiledFunction().getC());
        }
    }

    @Override public void run() throws Exception {
        DecompInterface d = new DecompInterface();
        d.openProgram(currentProgram);
        long vtable = 0x005E2100L;
        Set<Long> functions = new LinkedHashSet<>();
        println("=== MINEBOT ATTACK VTABLE 0x005E2100 ===");
        for (int off = 0; off <= 0x50; off += 4) {
            long p = ptr(vtable + off);
            println(String.format("+%02X -> %08X", off, p));
            if (p >= 0x00400000L && p < 0x00590000L) functions.add(p);
        }
        for (long p : functions) dump(p, d);
        dump(0x004508A0L, d);
        dump(0x00450E80L, d);
        d.dispose();
    }
}
