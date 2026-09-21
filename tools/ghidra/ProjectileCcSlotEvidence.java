import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Instruction;

public class ProjectileCcSlotEvidence extends GhidraScript {
    private long u32(long a) throws Exception { return Integer.toUnsignedLong(getInt(toAddr(a))); }
    @Override public void run() throws Exception {
        long base=0x005DF2B0L, mine=0x005DF390L;
        for(int off=0xC0;off<0xE0;off+=4) {
            println(String.format("slot +0x%02X base=0x%08X mine=0x%08X",off,u32(base+off),u32(mine+off)));
        }
        int hits=0;
        Instruction ins=currentProgram.getListing().getInstructions(true).next();
        while(ins!=null){
            String s=ins.toString().toLowerCase();
            if(ins.getMnemonicString().equalsIgnoreCase("CALL") && (s.contains("+ 0xcc]") || s.contains("+0xcc]"))){
                println(ins.getAddress()+" "+ins); hits++;
            }
            ins=ins.getNext();
        }
        println("indirect +0xCC call hits="+hits);
    }
}
