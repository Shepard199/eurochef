import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.InstructionIterator;
import ghidra.program.model.listing.Listing;
import ghidra.program.model.symbol.Reference;

public class Stage536Evidence extends GhidraScript {
    private void dumpLinear(long raw, int maxInstructions) throws Exception {
        Address start = toAddr(raw);
        Listing listing = currentProgram.getListing();
        disassemble(start);
        println(String.format("TARGET 0x%08X", raw));
        InstructionIterator it = listing.getInstructions(start, true);
        int count = 0;
        while (it.hasNext() && count < maxInstructions) {
            Instruction ins = it.next();
            println("  " + ins.getAddress() + "  " + ins.toString());
            for (Reference ref : ins.getReferencesFrom()) {
                println("    REF " + ref.getReferenceType() + " -> " + ref.getToAddress());
            }
            count++;
        }
        println("  instructions_dumped=" + count);
    }

    @Override
    public void run() throws Exception {
        println("PROGRAM=" + currentProgram.getName());
        dumpLinear(0x004CFE00L, 40);
        Address table = toAddr(0x004D00ACL);
        for (int i = 0; i < 4; i++) {
            long target = Integer.toUnsignedLong(getInt(table.add(i * 4L)));
            println(String.format("PHASE%d_TARGET=0x%08X", i, target));
            dumpLinear(target, 90);
        }
        dumpLinear(0x00509C48L, 20);
    }
}
