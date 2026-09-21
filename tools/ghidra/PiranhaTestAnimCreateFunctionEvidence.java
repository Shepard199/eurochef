import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;

public class PiranhaTestAnimCreateFunctionEvidence extends GhidraScript {
    private Function ensure(long raw) throws Exception {
        Address a = toAddr(raw);
        Function f = getFunctionAt(a);
        if (f != null) return f;
        disassemble(a);
        try {
            f = createFunction(a, null);
        } catch (Exception e) {
            println(String.format("createFunction 0x%08X failed: %s", raw, e.getMessage()));
        }
        if (f == null) f = getFunctionContaining(a);
        return f;
    }

    private void dump(long raw, DecompInterface d) throws Exception {
        Function f = ensure(raw);
        println(String.format("\n=== 0x%08X %s ===", raw, f == null ? "<missing>" : f.getName()));
        if (f == null) return;
        DecompileResults r = d.decompileFunction(f, 90, monitor);
        if (r.decompileCompleted() && r.getDecompiledFunction() != null) {
            println(r.getDecompiledFunction().getC());
        }
    }

    @Override
    public void run() throws Exception {
        DecompInterface d = new DecompInterface();
        d.openProgram(currentProgram);
        long[] targets = {
            0x004676F0L,
            0x00467C60L,
            0x00468590L,
            0x0046D840L,
            0x00450CB0L,
            0x00467580L
        };
        for (long t : targets) dump(t, d);
        d.dispose();
    }
}
