import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;

public class StandardMonsterUniqueUpdateEvidence extends GhidraScript {
    private void recover(long raw, String name, DecompInterface d) throws Exception {
        Address a = toAddr(raw);
        disassemble(a);
        Function f = getFunctionAt(a);
        if (f == null) f = createFunction(a, name);
        println(String.format("\n=== 0x%08X %s ===", raw, f == null ? "<missing>" : f.getName()));
        if (f == null) return;
        DecompileResults r = d.decompileFunction(f, 90, monitor);
        if (r.decompileCompleted() && r.getDecompiledFunction() != null) {
            println(r.getDecompiledFunction().getC());
        }
    }
    @Override public void run() throws Exception {
        DecompInterface d = new DecompInterface();
        d.openProgram(currentProgram);
        recover(0x00465E30L, "tmp_EB13_KnightBot_Update", d);
        recover(0x004647B0L, "tmp_EB14_Minion_Update", d);
        recover(0x00460340L, "tmp_EW10_Minion_Update", d);
        d.dispose();
    }
}