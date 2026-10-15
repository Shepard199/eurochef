import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;

public class PlayToStateMarkerOwnerEvidence extends GhidraScript {
    private void dump(long raw, DecompInterface d) throws Exception {
        Address a = toAddr(raw);
        Function f = getFunctionAt(a);
        println(String.format("\n=== 0x%08X %s ===", raw, f == null ? "<missing>" : f.getName()));
        if (f == null) return;
        DecompileResults r = d.decompileFunction(f, 90, monitor);
        if (r.decompileCompleted() && r.getDecompiledFunction() != null) println(r.getDecompiledFunction().getC());
    }
    public void run() throws Exception {
        DecompInterface d = new DecompInterface();
        d.openProgram(currentProgram);
        dump(0x00403B50L, d);
        dump(0x00417CB0L, d);
        d.dispose();
    }
}
