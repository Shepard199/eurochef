import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;

public class BaseMonsterTailEvidence extends GhidraScript {
    private long ptr(long a) throws Exception {
        return Integer.toUnsignedLong(getInt(toAddr(a)));
    }

    private void decompile(long raw, DecompInterface d) throws Exception {
        Function f = getFunctionAt(toAddr(raw));
        if (f == null) f = getFunctionContaining(toAddr(raw));
        println(String.format("\n=== 0x%08X %s ===", raw, f == null ? "<missing>" : f.getName()));
        if (f == null) return;
        DecompileResults r = d.decompileFunction(f, 60, monitor);
        if (r.decompileCompleted() && r.getDecompiledFunction() != null) {
            println(r.getDecompiledFunction().getC());
        }
    }

    private void descriptor(String name, long raw) throws Exception {
        println(String.format("\n=== %s descriptor 0x%08X ===", name, raw));
        for (int off = 0; off < 0x20; off += 4) {
            println(String.format("+0x%02X = 0x%08X", off, ptr(raw + off)));
        }
    }

    @Override
    public void run() throws Exception {
        descriptor("EM07_PiranhaBot", 0x005E28ECL);
        descriptor("TestAnimBot", 0x005E28FCL);
        println(String.format("\nbase vtable +0x108 -> 0x%08X", ptr(0x005E2920L + 0x108)));

        DecompInterface d = new DecompInterface();
        d.openProgram(currentProgram);
        decompile(0x0045AB60L, d);
        decompile(0x0045ACC0L, d);
        decompile(0x004190D0L, d);
        decompile(0x004514F0L, d);
        d.dispose();
    }
}
