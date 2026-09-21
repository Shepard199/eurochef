import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.mem.Memory;

public class FishCreatorVtableSearchEvidence extends GhidraScript {
    private long ptr(long a)throws Exception{return Integer.toUnsignedLong(getInt(toAddr(a)));}
    @Override public void run()throws Exception{
        Memory mem=currentProgram.getMemory();
        byte[] needle=new byte[]{0x60,0x01,0x48,0x00};
        Address start=currentProgram.getMinAddress();
        while(true){
            Address hit=mem.findBytes(start,needle,null,true,monitor);
            if(hit==null)break;
            long slot=hit.getOffset();
            long base=slot-0x10c;
            if(base>=currentProgram.getMinAddress().getOffset() && base+0x110<=currentProgram.getMaxAddress().getOffset()){
                long next=ptr(base+0x110);
                println(String.format("candidate base 0x%08X +10C=0x%08X +110=0x%08X",base,ptr(base+0x10c),next));
            }
            start=hit.add(1);
        }
        needle=new byte[]{0x70,0x01,0x48,0x00};
        start=currentProgram.getMinAddress();
        while(true){
            Address hit=mem.findBytes(start,needle,null,true,monitor);
            if(hit==null)break;
            long slot=hit.getOffset();
            long base=slot-0x110;
            if(base>=currentProgram.getMinAddress().getOffset() && base+0x110<=currentProgram.getMaxAddress().getOffset()){
                println(String.format("candidate2 base 0x%08X +10C=0x%08X +110=0x%08X",base,ptr(base+0x10c),ptr(base+0x110)));
            }
            start=hit.add(1);
        }
    }
}
