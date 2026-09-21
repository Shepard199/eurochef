import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;

public class MineBotAttackUnknownEvidence extends GhidraScript {
    private void dump(long raw, DecompInterface d) throws Exception {
        Address a = toAddr(raw);
        println(String.format("\n=== 0x%08X ===", raw));
        Function f = getFunctionContaining(a);
        if (f != null) {
            println("FUNCTION=" + f.getName() + " ENTRY=" + f.getEntryPoint());
            DecompileResults r = d.decompileFunction(f, 60, monitor);
            if (r.decompileCompleted() && r.getDecompiledFunction() != null) {
                println(r.getDecompiledFunction().getC());
                return;
            }
        }
        Instruction ins = getInstructionAt(a);
        if (ins == null) {
            disassemble(a);
            ins = getInstructionAt(a);
        }
        for (int i = 0; ins != null && i < 180; i++) {
            println(ins.getAddress() + " " + ins);
            if (ins.getMnemonicString().equalsIgnoreCase("RET")) break;
            ins = ins.getNext();
        }
    }

    @Override public void run() throws Exception {
        DecompInterface d = new DecompInterface();
        d.openProgram(currentProgram);
        dump(0x00455DD0L, d);
        dump(0x00450AB0L, d);
        dump(0x00450C60L, d);
        d.dispose();
    }
}
