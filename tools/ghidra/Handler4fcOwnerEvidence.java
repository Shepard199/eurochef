import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;

public class Handler4fcOwnerEvidence extends GhidraScript {
    private void dc(long a, DecompInterface d) throws Exception {
        Function f = getFunctionContaining(toAddr(a));
        println(String.format("\n=== %08X %s ===", a, f == null ? "<missing>" : f.getName()));
        if (f == null) return;
        DecompileResults r = d.decompileFunction(f, 90, monitor);
        if (r.decompileCompleted() && r.getDecompiledFunction() != null) {
            println(r.getDecompiledFunction().getC());
        }
    }
    @Override public void run() throws Exception {
        DecompInterface d = new DecompInterface();
        d.openProgram(currentProgram);
        long[] funcs = {0x00451350L,0x004547E0L,0x00454E60L,0x00454EE0L,0x004571E0L};
        for (long a : funcs) dc(a,d);
        d.dispose();
    }
}