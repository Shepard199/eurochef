import ghidra.app.script.GhidraScript;
import ghidra.program.model.mem.Memory;
public class RobotsInputDefaultFullTableEvidence extends GhidraScript {
 public void run()throws Exception{
  Memory m=currentProgram.getMemory();
  long base=0x005e1758L;
  for(int a=0;a<0x30;a++){
   long v=Integer.toUnsignedLong(m.getInt(toAddr(base+a*4L)));
   println(String.format("action=0x%02X default=0x%08X (%d)",a,v,v));
  }
 }
}