import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;

public class AiResidualTempFunctionEvidence extends GhidraScript {
    private void recover(long raw, String name, DecompInterface d) throws Exception {
        Address a = toAddr(raw);
        disassemble(a);
        Function f = getFunctionAt(a);
        if (f == null) {
            f = createFunction(a, name);
        }
        println(String.format("\n=== 0x%08X %s ===", raw, f == null ? "<missing>" : f.getName()));
        if (f == null) return;
        DecompileResults r = d.decompileFunction(f, 90, monitor);
        if (r.decompileCompleted() && r.getDecompiledFunction() != null) {
            println(r.getDecompiledFunction().getC());
        } else {
            println("DECOMPILE_FAILED");
        }
    }

    @Override
    public void run() throws Exception {
        DecompInterface d = new DecompInterface();
        d.openProgram(currentProgram);
        recover(0x004676F0L, "tmp_Piranha_Update", d);
        recover(0x00467C60L, "tmp_Piranha_FirstUpdate", d);
        recover(0x00468590L, "tmp_TestAnim_Builder", d);
        d.dispose();
    }
}
