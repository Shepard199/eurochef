import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
import java.util.LinkedHashSet;
import java.util.Set;

public class PiranhaTestAnimVtableEvidence extends GhidraScript {
    private long ptr(long a) throws Exception {
        return Integer.toUnsignedLong(getInt(toAddr(a)));
    }

    private void diff(String name, long base, long child, int bytes, Set<Long> funcs) throws Exception {
        println(String.format("\n=== %s DIFF 0x%08X -> 0x%08X ===", name, base, child));
        for (int off = 0; off < bytes; off += 4) {
            long a = ptr(base + off);
            long b = ptr(child + off);
            if (a != b) {
                println(String.format("+0x%03X: 0x%08X -> 0x%08X", off, a, b));
                if (b >= 0x00400000L && b < 0x00590000L) funcs.add(b);
            }
        }
    }

    private void decompile(long raw, DecompInterface d) throws Exception {
        Function f = getFunctionAt(toAddr(raw));
        if (f == null) f = getFunctionContaining(toAddr(raw));
        println(String.format("\n--- 0x%08X %s ---", raw, f == null ? "<missing>" : f.getName()));
        if (f == null) return;
        DecompileResults r = d.decompileFunction(f, 60, monitor);
        if (r.decompileCompleted() && r.getDecompiledFunction() != null) {
            println(r.getDecompiledFunction().getC());
        }
    }

    @Override
    public void run() throws Exception {
        Set<Long> funcs = new LinkedHashSet<>();
        diff("EM07_PiranhaBot", 0x005E2920L, 0x005E6A40L, 0x170, funcs);
        diff("TestAnimBot", 0x005E2920L, 0x005E6BA8L, 0x170, funcs);

        DecompInterface d = new DecompInterface();
        d.openProgram(currentProgram);
        for (long f : funcs) decompile(f, d);
        d.dispose();
    }
}
