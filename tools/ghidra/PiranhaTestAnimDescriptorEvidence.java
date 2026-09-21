import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;

public class PiranhaTestAnimDescriptorEvidence extends GhidraScript {
    private void dec(long raw, DecompInterface d) throws Exception {
        Function f = getFunctionAt(toAddr(raw));
        if (f == null) f = getFunctionContaining(toAddr(raw));
        println(String.format("\n=== 0x%08X %s ===", raw, f == null ? "<missing>" : f.getName()));
        if (f == null) return;
        DecompileResults r = d.decompileFunction(f, 60, monitor);
        if (r.decompileCompleted() && r.getDecompiledFunction() != null) println(r.getDecompiledFunction().getC());
    }
    @Override public void run() throws Exception {
        DecompInterface d = new DecompInterface();
        d.openProgram(currentProgram);
        dec(0x0045AC10L, d);
        dec(0x0045AD30L, d);
        dec(0x004514F0L, d);
        dec(0x004190D0L, d);
        d.dispose();
    }
}
