import ghidra.app.script.GhidraScript;

public class FishVtableSlotMapEvidence extends GhidraScript {
    private long ptr(long a)throws Exception{return Integer.toUnsignedLong(getInt(toAddr(a)));}
    @Override public void run()throws Exception{
        long vt=0x005E83E0L;
        for(int off=0;off<=0x220;off+=4){
            long f=ptr(vt+off);
            if(f==0x00480140L||f==0x00480150L||f==0x00480160L||f==0x00480170L||f==0x0044CD10L){
                println(String.format("MATCH +0x%03X -> 0x%08X",off,f));
            }
        }
    }
}
