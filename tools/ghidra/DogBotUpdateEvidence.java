import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Instruction;

public class DogBotUpdateEvidence extends GhidraScript {
    @Override public void run() throws Exception {
        Address start = toAddr(0x00460870L);
        Address end = toAddr(0x00460A00L);
        println("=== DogBot vslot +0x34 linear disassembly 0x00460870..0x00460A00 ===");
        Instruction insn = getInstructionAt(start);
        if (insn == null) insn = getInstructionAfter(start.subtract(1));
        int count = 0;
        while (insn != null && insn.getAddress().compareTo(end) < 0 && count++ < 256) {
            println(insn.getAddress() + "  " + insn.toString());
            insn = insn.getNext();
        }
    }
}
