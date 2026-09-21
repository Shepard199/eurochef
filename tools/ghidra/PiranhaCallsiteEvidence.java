import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.InstructionIterator;
import ghidra.program.model.symbol.Reference;

public class PiranhaCallsiteEvidence extends GhidraScript {
    private void dump(long raw, int count) throws Exception {
        Address a = toAddr(raw);
        disassemble(a);
        println(String.format("\n=== 0x%08X ===", raw));
        InstructionIterator it = currentProgram.getListing().getInstructions(a, true);
        int n=0;
        while(it.hasNext() && n<count) {
            Instruction ins=it.next();
            println(ins.getAddress()+"  "+ins);
            for (Reference ref:ins.getReferencesFrom()) {
                println("    REF "+ref.getReferenceType()+" -> "+ref.getToAddress());
            }
            n++;
        }
    }
    @Override public void run() throws Exception {
        dump(0x00467980L, 120);
        dump(0x00467C60L, 180);
    }
}
