import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.InstructionIterator;
import ghidra.program.model.symbol.Reference;

public class AiResidualLinearEvidence extends GhidraScript {
    private void dump(long raw, int max) throws Exception {
        Address start = toAddr(raw);
        disassemble(start);
        println(String.format("\n=== 0x%08X ===", raw));
        InstructionIterator it = currentProgram.getListing().getInstructions(start, true);
        int count = 0;
        while (it.hasNext() && count < max) {
            Instruction ins = it.next();
            println(ins.getAddress() + "  " + ins.toString());
            for (Reference ref : ins.getReferencesFrom()) {
                println("    REF " + ref.getReferenceType() + " -> " + ref.getToAddress());
            }
            count++;
        }
        println("COUNT=" + count);
    }

    @Override
    public void run() throws Exception {
        dump(0x004676F0L, 140);
        dump(0x00467C60L, 120);
        dump(0x00468590L, 140);
    }
}
