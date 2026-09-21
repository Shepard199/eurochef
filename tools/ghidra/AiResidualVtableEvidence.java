import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
import java.util.LinkedHashSet;
import java.util.Set;

public class AiResidualVtableEvidence extends GhidraScript {
    private long ptr(long a) throws Exception {
        return Integer.toUnsignedLong(getInt(toAddr(a)));
    }

    private void diff(String name, long base, long derived, int bytes, DecompInterface d) throws Exception {
        println(String.format("\n=== %s 0x%08X vs 0x%08X ===", name, base, derived));
        Set<Long> funcs = new LinkedHashSet<>();
        for (int off = 0; off < bytes; off += 4) {
            long a = ptr(base + off);
            long b = ptr(derived + off);
            if (a != b) {
                println(String.format("+0x%03X: 0x%08X -> 0x%08X", off, a, b));
                if (b >= 0x00400000L && b < 0x00590000L) funcs.add(b);
            }
        }
        for (long target : funcs) {
            Function f = getFunctionContaining(toAddr(target));
            println(String.format("\n--- 0x%08X %s ---", target, f == null ? "<missing>" : f.getName()));
            if (f == null) continue;
            DecompileResults r = d.decompileFunction(f, 60, monitor);
            if (r.decompileCompleted() && r.getDecompiledFunction() != null) {
                println(r.getDecompiledFunction().getC());
            }
        }
    }

    @Override
    public void run() throws Exception {
        DecompInterface d = new DecompInterface();
        d.openProgram(currentProgram);
        diff("PiranhaBot", 0x005E2920L, 0x005E6A40L, 0x170, d);
        diff("TestAnimBot", 0x005E2920L, 0x005E6BA8L, 0x170, d);
        d.dispose();
    }
}
