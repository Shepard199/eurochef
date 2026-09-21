import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;

public class AiAttackRecordLifecycleEvidence extends GhidraScript {
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
        if (getInstructionAt(a) == null) disassemble(a);
        Instruction ins = getInstructionAt(a);
        for (int i=0; ins != null && i<260; i++) {
            println(ins.getAddress()+" "+ins);
            if (ins.getMnemonicString().equalsIgnoreCase("RET") && i > 5) break;
            ins=ins.getNext();
        }
    }
    @Override public void run() throws Exception {
        DecompInterface d=new DecompInterface(); d.openProgram(currentProgram);
        long[] addrs={0x004511E0L,0x00451350L,0x004517A0L,0x00453160L,0x00454E60L,0x00454EE0L,0x00455DC0L,0x00456DE0L};
        for(long a:addrs) dump(a,d);
        d.dispose();
    }
}
