import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;

public class PiranhaUpdateEvidence extends GhidraScript {
    @Override
    public void run() throws Exception {
        disassemble(toAddr(0x004676F0L));
        Function f = getFunctionAt(toAddr(0x004676F0L));
        if (f == null) {
            try { f = createFunction(toAddr(0x004676F0L), null); } catch (Exception ignored) {}
        }
        if (f == null) f = getFunctionContaining(toAddr(0x004676F0L));
        println("FUNCTION=" + (f == null ? "<missing>" : f.getName()));
        if (f == null) return;
        DecompInterface d = new DecompInterface();
        d.openProgram(currentProgram);
        DecompileResults r = d.decompileFunction(f, 120, monitor);
        if (r.decompileCompleted() && r.getDecompiledFunction() != null) {
            println(r.getDecompiledFunction().getC());
        }
        d.dispose();
    }
}
