import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
public class RobotsInputInitAsmEvidence extends GhidraScript {
 public void run()throws Exception{
  Listing l=currentProgram.getListing();
  long[] ranges={0x004d18e0L,0x004d1610L};
  long[] ends={0x004d19d0L,0x004d1800L};
  for(int j=0;j<ranges.length;j++){
   println(String.format("\n=== ASM 0x%08X ===",ranges[j]));
   InstructionIterator it=l.getInstructions(toAddr(ranges[j]),true);
   while(it.hasNext()){
    Instruction ins=it.next();
    if(ins.getAddress().compareTo(toAddr(ends[j]))>=0)break;
    println(ins.getAddress()+"  "+ins);
   }
  }
 }
}